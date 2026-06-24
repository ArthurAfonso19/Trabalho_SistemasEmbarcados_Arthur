use defmt::info;
use embassy_stm32::{mode, usart::Uart};
use heapless::{String, Vec};

//use crate::app::shell::RxState::Receiving;


use crate::app::shell;

pub const RX_BUF_SIZE: usize = 64;
pub const MAX_ARGS: usize = 8;

pub const  RESPONSE_SIZE: usize = 256; 
// Reposta textual gerada pela shell
//Neste passo ainda não escrevemos direto na UART
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
    Utf8Error,
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

//Tabela minima de comandos da shell nesta fase
pub const  COMMANDS: &[CommandEntry] = &[
    CommandEntry{
        name: "help",
        help: "Lista os comandos disponíveis",
    },
    CommandEntry{
        name: "status",
        help: "Exibe um breve estado do sistema",
    },
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

    pub fn state(&self) -> RxState
    {
        self.state
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
            // Se vier CR, considera fim de linha.
            b'\r' => {
                self.state = RxState::LineReady;
                Ok(())
            }

            // Se vier LF sozinho, tambem fecha a linha.
            // Se vier depois de um CRLF, o buffer pode ja estar vazio,
            // entao ignoramos esse LF extra.
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

//Executa um comando ja parseado e devolve a resposta textual 
pub fn execute_command(cmd: &ParsedCommand) -> ShellResponse
{
    let mut out = ShellResponse::new();

    match cmd.command{
        "help" => {
            let _ = out.push_str("Comandos disponíveis: \r\n");

            for entry in COMMANDS {
                let _ = out.push_str(" - ");
                let _ = out.push_str(entry.name);
                let _ = out.push_str(": ");
                let _ = out.push_str(entry.help);
                let _ = out.push_str("\r\n");
            }
        }

        "status" => {
            //Nesta fase, o status ainda é simples e fixo 
            //Mais para frente pode mostrar uptimes, tarefas e memória 
            let _ = out.push_str("Sistema ativo\r\n");
            let _ = out.push_str("UART: ok\r\n");
            let _ = out.push_str("Shell: inicial\r\n");
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
                //Se o usuário apertou Backspace/Delete e há algo no buffer,
                //  apagamos do buffer e também da tela 
                if shell.pop_last().is_some()
                {
                    echo_input_byte(&mut uart, byte).await;
                }
                continue;
            }

            b'\n' => {
                //Ignora LF sozinho para evitar eco duplo em terminais CRLF
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

        //Alimenta o buffer da shell com o byte recebido s
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
                    let response = execute_command(&cmd);
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




