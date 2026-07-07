//use cortex_m::register::control;
use defmt::info;
use embassy_stm32::usart::Uart;
use heapless::{String, Vec};
use crate::app::monitor::{self, TaskMetrics};
use crate::drivers::hcsr04;
use crate::{LED_CONTROL, MONITOR};
use embassy_time::Instant;
use core::error;
use core::fmt::Write;

pub const RX_BUF_SIZE: usize = 64;
pub const MAX_ARGS: usize = 8;
pub const  RESPONSE_SIZE: usize = 512; 
// Reposta textual gerada pela shell
pub type  ShellResponse = String<RESPONSE_SIZE>;


//Estado simples da recepção da shell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxState 
{
    Receiving,
    LineReady,
    Overflow,
}

//Possíveis erros de parser/recepção 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellError
{
    BufferFull,
    TooManyArgs,
}

//Resultado do parser 
// Os '&str' apontam para a própria linha recebida 
pub struct ParsedCommand<'a>
{
    pub command: &'a str,
    pub args: Vec<& 'a str, MAX_ARGS>,
}

// Entrada simples da tabela de comandos
pub struct CommandEntry
{
    pub name: &'static str,
    pub help: &'static str,
}

//Tabela de comandos da shell 
pub const  COMMANDS: &[CommandEntry] = &[
    CommandEntry
    {
        name: "help",
        help: "Lista os comandos disponíveis",
    },

    CommandEntry
    {
        name: "status",
        help: "Exibe um breve estado do sistema",
    },

    CommandEntry
    {
        name: "tasks",
        help: "Lista de tarefas com métricas",
    },

    CommandEntry
    {
        name: "uptime",
        help: "Mostra o tempo desde o boot",
    },

    CommandEntry{
        name: "mem",
        help: "Mostra o tamanho das seções de memória",
    },

    CommandEntry{
        name: "led",
        help: "Controla o LED: on, off, period <ms>",
    },

    CommandEntry{
        name: "sensors",
        help: "Mostra a ultima leitura dos sensores",
    },

    CommandEntry{
        name: "clean",
        help: "Limpa o terminal",
    },

    CommandEntry{
        name: "rt",
        help: "Exibe métricas de tempo real: intervalo, jitter, execucoes",
    }
];

//Buffer circular simples para receber bytes da UART
pub struct ShellBuffer
{
    buf: [u8; RX_BUF_SIZE],
    head: usize,
    tail: usize,
    len: usize,
    state: RxState,
}

impl ShellBuffer
{
    pub const fn new() -> Self {
        Self{
            buf: [0; RX_BUF_SIZE],
            head: 0,
            tail: 0,
            len: 0,
            state: RxState::Receiving,
        }
    }

    pub fn clear(&mut self)
    {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
        self.state = RxState::Receiving; 
    }

    fn push_raw(&mut self, byte:u8) ->Result<(), ShellError>
    {
        if self.len == RX_BUF_SIZE
        {
            self.state = RxState::Overflow;
            return Err(ShellError::BufferFull);
        }

        self.buf[self.tail] = byte;
        self.tail = (self.tail + 1) % RX_BUF_SIZE;
        self.len += 1;
        Ok(())
    }

    //Quando o usuário aperta "Backspace", não basta apagar visualmente
    //Também é preciso remover o último byte armazenado no buffer da shell
    fn pop_last(&mut self) -> Option<u8>
    {
        //Se houver bytes acumaldos, não há o que apagar 
        if self.len == 0
        {
            return None;
        }

        //Volta uma posição na cauda do buffer circular 
        self.tail = (self.tail + RX_BUF_SIZE - 1) % RX_BUF_SIZE;
        let byte = self.buf[self.tail];
        self.len -= 1;
        Some(byte)
    }

    fn pop_raw(&mut self) -> Option<u8>
    {
        if self.len == 0
        {
            return  None;
        }

        let byte = self.buf[self.head]; 
        self.head = (self.head + 1) % RX_BUF_SIZE;
        self.len -= 1;
        Some(byte)
    }

    //Recebe um byte da UART
    //Regras:
    //- '\r' é ignorado
    //- '\n' marca fim de linha 
    //- outros bytes entram no buffer circular 
    pub fn push_byte(&mut self, byte: u8) -> Result<(), ShellError>
    {
        if self.state != RxState::Receiving
        {
            return Ok(());
        }

        match byte {
            b'\r' => {
                self.state = RxState::LineReady;
                Ok(())
            }

            b'\n' => {
                if self.len == 0 {
                    Ok(())
                } else {
                    self.state = RxState::LineReady;
                    Ok(())
                }
            }

            _ => self.push_raw(byte),
        }
    }

    //Extrai a linha pronta do buffer circular para uma 'String<64>'
    pub fn take_line(&mut self) -> Result<Option<String<RX_BUF_SIZE>>, ShellError>
    {
        if self.state != RxState::LineReady
        {
            return  Ok(None);
        }

        let mut  line: String<RX_BUF_SIZE> = String::new();

        while let Some(byte) = self.pop_raw() 
        {
            if line.push(byte as char).is_err()
            {
                self.clear();
                return Err(ShellError::BufferFull);
            }    
        }

        self.state = RxState::Receiving;
        Ok(Some(line))
    }
}

//Faz parse de uma linha no formato 
// 'comando arg1 arg2 arg3'
pub fn parse_command<'a>(line: &'a str) -> Result<Option<ParsedCommand<'a>>, ShellError>
{
    let trimmed = line.trim();

    if trimmed.is_empty()
    {
        return  Ok(None);
    }

    let mut parts = trimmed.split_ascii_whitespace();

    let command = match  parts.next() 
    {
        Some(cmd) => cmd,
        None => return Ok(None),    
    };

    let mut args: Vec<&str, MAX_ARGS> = Vec::new();

    for part in parts
    {
        if args.push(part).is_err()
        {
            return  Err(ShellError::TooManyArgs);
        }
    }

    Ok(Some(ParsedCommand { command, args }))
}

fn write_task_metrics_line(out: &mut ShellResponse, metrics: TaskMetrics)
{
    let _ = out.push_str("- ");
    let _ = out.push_str(metrics.name);
    let _ = out.push_str(": count=");
    let _ = write!(out, "{}", metrics.execution_count);
    let _ = out.push_str(", last_ms=");
    let _ = write!(out, "{:?}", metrics.last_run_ms);
    let _ = out.push_str(", min_ms=");
    let _ = write!(out, "{:?}", metrics.min_interval_ms);
    let _ = out.push_str(", max_ms=");
    let _ = write!(out, "{:?}\r\n", metrics.max_interval_ms);
}

//Executa um comando ja parseado e devolve a resposta textual 
pub async fn execute_command(cmd: &ParsedCommand<'_>) -> ShellResponse
{
    let mut out = ShellResponse::new();

    match cmd.command{
        "help" => {
            if !cmd.args.is_empty() {
                let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } else {
                let _ = out.push_str("Comandos disponíveis: \r\n");

                for entry in COMMANDS {
                    let _ = out.push_str(" - ");
                    let _ = out.push_str(entry.name);
                    let _ = out.push_str(": ");
                    let _ = out.push_str(entry.help);
                    let _ = out.push_str("\r\n");
                }
            }
        }

        "status" => {
            if !cmd.args.is_empty() {
                let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } else {
                let _ = out.push_str("Sistema ativo\r\n");
                let _ = out.push_str("UART: ok\r\n");
                let _ = out.push_str("Shell: inicial\r\n");
            }
        }

        "tasks" => {
            if !cmd.args.is_empty() {
                let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } else {
                //Copia as metricas e libera o lock antes de formatar a resposta.
                let (adc, button, led) = {
                    let monitor = MONITOR.lock().await;
                    (monitor.adc, monitor.button, monitor.led)
                };

                let _ = out.push_str("Tasks monitoradas: \r\n");

                write_task_metrics_line(&mut out, adc);
                write_task_metrics_line(&mut out, button);
                write_task_metrics_line(&mut out, led);
            }
        }

        "uptime" => {
            if !cmd.args.is_empty() {
                let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } else {
                //Lê o tempo atual desde o boot 
                let uptime_ms = Instant::now().as_millis();
 
                let _ = out.push_str("Uptime: ");
                let _ = write!(out, "{}", uptime_ms);
                let _ = out.push_str(" ms\r\n");
            }
        }

        "mem" => {
            if !cmd.args.is_empty() 
            {
            // Mantem o comportamento consistente com help, status, tasks e uptime.
            let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } 
            else 
            {
                // Le os tamanhos calculados a partir dos simbolos do linker.
                let (text_size, data_size, bss_size) = crate::firmware_memory_sizes();

                // Cabecalho simples da resposta.
                let _ = out.push_str("Memoria do firmware:\r\n");

                // Mostra o tamanho da secao de codigo.
                let _ = out.push_str("- .text: ");
                let _ = write!(out, "{}", text_size);
                let _ = out.push_str(" bytes\r\n");

                // Mostra o tamanho da secao de dados inicializados.
                let _ = out.push_str("- .data: ");
                let _ = write!(out, "{}", data_size);
                let _ = out.push_str(" bytes\r\n");

                // Mostra o tamanho da secao de dados zerados.
                let _ = out.push_str("- .bss: ");
                let _ = write!(out, "{}", bss_size);
                let _ = out.push_str(" bytes\r\n");
            }
        }

        "clean" => {
            if !cmd.args.is_empty() 
            {
                let _ = out.push_str("Erro: este comando nao aceita argumentos. \r\n");
            }
            else 
            {
                let _ = out.push_str("\x1B[2J\x1B[H");   
            }
        }

        "led" => {
            match  cmd.args.as_slice() {
                ["on"] => {
                    let mut control = LED_CONTROL.lock().await;
                    control.set_enabled(true);
                    let _ = out.push_str("LED ligado. \r\n");
                }

                ["off"] => {
                    let mut control = LED_CONTROL.lock().await;
                    control.set_enabled(false);
                    let _ = out.push_str("LED desligado.\r\n");
                }

                ["period", value] => {
                    match value.parse::<u32>() 
                    {
                        Ok(period_ms) if period_ms >= 2 => {
                            let mut control = LED_CONTROL.lock().await;
                            control.set_period(period_ms);

                            let _ = out.push_str("Periodo do LED ajustado para ");
                            let _ = write!(out, "{}", period_ms);
                            let _ = out.push_str(" ms\r\n");
                        }

                        Ok(_) => {
                            let _ = out.push_str("Erro: o periodo deve ser maior ou igual a 2 ms.\r\n");
                        }

                        Err(_) => {
                            let _ = out.push_str("Erro: periodo invalido. Use um numero inteiro em ms.\r\n");
                        }
                    }
                }

                _ => {
                    //Qualquer outro formato cai em erro de uso
                    let _ = out.push_str("Uso: led on | led off | led period <ms>\r\n");
                }
            }
        }

        "sensors" => {
            //Mantem o padrao dos outros comandos: nada de argumentos extras 
            if !cmd.args.is_empty()
            {
                let _ = out.push_str("Erro: este comando não aceita argumentos.\r\n");
            }
            else 
            {
                //Copia snapshot e libera o lock antes de formatar a resposta 
                let (am2302, hcsr04) = {
                    let monitor = MONITOR.lock().await;
                    (monitor.am2302, monitor.hcsr04)
                };

                let _ = out.push_str("Sensores:\r\n"); 

                match (am2302.temperature_c, am2302.humidity_rh, am2302.last_update_ms) 
                {
                    (Some(temperature_c), Some(humidity_rh), Some(last_update_ms)) => {
                        //Mostra a ultima temperatura válida 
                        let _ = out.push_str("AM2302 temp: ");
                        let _ = write!(out, "{}", temperature_c);
                        let _ = out.push_str(" C\r\n");
                        
                        //Mostra a ultima umidade válida 
                        let _ = out.push_str("AM2302 humidity: ");
                        let _ = write!(out, "{}", humidity_rh);
                        let _ = out.push_str(" %RH\r\n");

                        //Calcula a quanto tempo essa leitura foi atualizada 
                        let now_ms = Instant::now().as_millis() as u64;
                        let age_ms = now_ms.saturating_sub(last_update_ms);
                        let _ = out.push_str("Atualizado ha: ");
                        let _ = write!(out, "{}", age_ms);
                        let _ = out.push_str(" ms\r\n");
                    } 

                    _ => {
                        //Ainda não houve leitura válida suficiente para mostrar dados. 
                        let _ = out.push_str("AM2302: sem leitura valida ainda\r\n");
                    }
                }

                if let Some(error) = am2302.last_error
                {
                    //Mostra o último erro conhecido, se existir 
                    let _ = out.push_str("Ultimo erro: ");
                    let _ = write!(out, "{:?}", error);
                    let _ = out.push_str("\r\n");
                }

                // --- HC-SR04 ---
                match (hcsr04.distance_cm, hcsr04.last_update_ms) {
                    (Some(distance_cm), Some(last_update_ms)) => {
                        let _ = out.push_str("HC-SR04 distance: ");
                        let _ = write!(out, "{}", distance_cm);
                        let _ = out.push_str(" cm\r\n");
                        let now_ms = Instant::now().as_millis() as u64;
                        let age_ms = now_ms.saturating_sub(last_update_ms);
                        let _ = out.push_str("Atualizado ha: ");
                        let _ = write!(out, "{}", age_ms);
                        let _ = out.push_str(" ms\r\n");
                    }
                    _ => {
                        let _ = out.push_str("HC-SR04: sem leitura valida ainda\r\n");
                    }
                }
                if let Some(error) = hcsr04.last_error {
                    let _ = out.push_str("Ultimo erro: ");
                    let _ = write!(out, "{:?}", error);
                    let _ = out.push_str("\r\n");
                }
            }
        }

        "rt" => {
            if !cmd.args.is_empty() {
                let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
            } else {
                // Copia as metricas e libera o lock antes de formatar.
                let (adc, button, led, am2302_task, hcsr04_task) = {
                    let monitor = MONITOR.lock().await;
                    (monitor.adc, monitor.button, monitor.led, monitor.am2302_task, monitor.hcsr04_task)
                };

                let _ = out.push_str("Tempo real:\r\n");

                // Funcao auxiliar local para formatar uma linha de task.
                fn format_rt_line(
                    out: &mut ShellResponse,
                    metrics: monitor::TaskMetrics,
                ) {
                    let _ = out.push_str("- ");
                    let _ = out.push_str(metrics.name);
                    let _ = out.push_str(": last=");
                    match metrics.last_interval_ms {
                        Some(ms) => { let _ = write!(out, "{}", ms); }
                        None => { let _ = out.push_str("N/A"); }
                    }
                    let _ = out.push_str("ms, expected=");
                    match metrics.expected_interval_ms {
                        Some(ms) => { let _ = write!(out, "{}", ms); }
                        None => { let _ = out.push_str("N/A"); }
                    }
                    let _ = out.push_str("ms, jitter_avg=");
                    match metrics.average_jitter_ms() {
                        Some(ms) => { let _ = write!(out, "{}", ms); }
                        None => { let _ = out.push_str("N/A"); }
                    }
                    let _ = out.push_str("ms, count=");
                    let _ = write!(out, "{}", metrics.execution_count);
                    let _ = out.push_str("\r\n");
                }

                format_rt_line(&mut out, adc);
                format_rt_line(&mut out, button);
                format_rt_line(&mut out, led);
                format_rt_line(&mut out, am2302_task);
                format_rt_line(&mut out, hcsr04_task);
            }
        }

        _ => {
            let _ = out.push_str("Comando desconhecido. Digite 'help'. \r\n");
        }
    }

    out
}

async fn echo_input_byte(
    uart: &mut Uart<'static, embassy_stm32::mode::Async>,
    byte: u8,
){
    match byte
    {
        b'\r' | b'\n' => {
            //Mostrar uma quebra de linha correta no terminal serial 
            let _ = uart.write(b"\r\n").await;
        }
        0x08 | 0x7F => {
            //Move para trás, apaga o caractere na tela e volta de novo 
            let _ = uart.write(b"\x08 \x08").await;
        }
        _ => {
            //Ecoa o byte normal para o usuário ver o que digitou 
            let _ = uart.write(&[byte]).await;
        }
    }
}

// Task da shell UART
//
// Responsabilidades:
// - ler bytes da UART
// - alimentar o buffer circular 
// - fazer parse do comando 
// - executar o comando 
// - escrever a resposta de volta na UART
#[embassy_executor::task]
pub async fn shell_taks(mut uart: Uart<'static, embassy_stm32::mode::Async>)
{
    let mut shell = ShellBuffer::new();
    let mut rx = [0u8; 1];

    info!("Shell UART iniciada");
    let _ = uart.write(b"Shell pronta. Digite 'help'.\r\n").await;

    loop 
    {
        //Le um byte por vez da UART
        uart.read(&mut rx).await.unwrap();
        let byte = rx[0];

        match byte 
        {
            0x08 | 0x7F => {
                //Se o usuário apertou Backspace/Delete e há algo no buffer, apagamos do buffer e também da tela 
                if shell.pop_last().is_some()
                {
                    echo_input_byte(&mut uart, byte).await;
                }
                continue;
            }

            b'\n' => {
                continue;
            }

            b'\r' => {
                //Ecoa Enter como quebra de linha visual no terminal 
                echo_input_byte(&mut uart, byte).await;
            }
            _ => {
                //Ecoa o caractere normal antes de processar a entrada 
                echo_input_byte(&mut uart, byte).await;
            }
        }

        //Alimenta o buffer da shell com o byte recebido 
        match shell.push_byte(rx[0])
        {
            Ok(()) => {}
            Err(ShellError::BufferFull) =>
            {
                //A resposta começa em nova linha para não grudar no texto digitado
                let _ = uart.write(b"Error: linha muito longa.\r\n").await;
                shell.clear();
                continue;
            }
            Err(_) => 
            {
                // Erro pequeno e direto para manter a shell limpa 
                let _ = uart.write(b"Erro de recepcao. \r\n").await;
                shell.clear();
                continue;
            }
        } 

        //Quando a linha estiver pronta, extrai, faz parse e responde
        match shell.take_line()
        {
            Ok(Some(line)) => match parse_command(line.as_str())
            {
                Ok(Some(cmd)) => {
                    let response = execute_command(&cmd).await;
                    let _ = uart.write(response.as_bytes()).await;
                }

                Ok(None) => {
                    //Linha vazia: nao responde nada
                }
                
                Err(ShellError::TooManyArgs) => {
                    let _ = uart.write(b"Erro: argumento demais.\r\n").await;
                }

                Err(_) => {
                    let _ = uart.write(b"Erro ao interpretar comando. \r\n").await;
                }
            }

            Ok(None) => {
                //Linha ainda não terminou 
            }
            Err(_) => {
                let _ = uart.write(b"Erro ao montar linha. \r\n").await;
            }
        }   
    }
}




