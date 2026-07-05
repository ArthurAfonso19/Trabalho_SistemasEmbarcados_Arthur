## Fase 5 - Passo 5: Expor o `AM2302` no comando `sensors`

Este arquivo agora esta escrito como guia de implementacao.

Objetivo deste passo:

- reaproveitar a leitura periodica do `AM2302` que ja existe
- guardar o ultimo resultado em memoria compartilhada
- criar o comando `sensors` na shell UART
- mostrar temperatura, umidade, idade da leitura e ultimo erro conhecido
- nao reler o sensor dentro da shell

---

## Ideia central

Com o driver atual, o fluxo do sensor ficou assim:

- `GPIO` faz o start do protocolo
- `TIM4_CH1` captura os timestamps das bordas
- `DMA1_CH5` grava esses timestamps em RAM
- a CPU decodifica os `40 bits` e valida o `checksum`

Isso significa que a shell **nao deve** chamar `sensor.read()`.

O correto neste passo e:

1. a task `am2302_task(...)` continua lendo o sensor periodicamente
2. essa task atualiza um snapshot compartilhado
3. o comando `sensors` apenas le esse snapshot e imprime a resposta

Essa arquitetura evita misturar a interface UART com o caminho de captura do sensor.

---

## Arquivos que voce vai mexer

Neste passo, a implementacao pode ficar concentrada em:

- `src/app/monitor.rs`
- `src/main.rs`
- `src/app/shell.rs`

Essa e a menor mudanca coerente com a estrutura atual do projeto.

---

## Resultado final esperado

Quando terminar, o comportamento deve ser este:

1. a task do `AM2302` continua rodando a cada `2 s`
2. quando a leitura der certo, o snapshot global e atualizado
3. quando a leitura der erro, o ultimo erro e registrado
4. o comando `help` passa a listar `sensors`
5. o comando `sensors` mostra os dados mais recentes sem travar a shell

Exemplo de saida esperada:

```text
Sensores:
AM2302 temp: 17.4 C
AM2302 humidity: 73.6 %RH
Atualizado ha: 580 ms
```

Ou, se houve erro recente depois de uma leitura valida:

```text
Sensores:
AM2302 temp: 17.4 C
AM2302 humidity: 73.6 %RH
Atualizado ha: 2140 ms
Ultimo erro: Timeout
```

---

## Passo 1: Expandir `src/app/monitor.rs`

### O que fazer

Hoje `monitor.rs` guarda apenas metricas de tasks.

Neste passo, ele tambem deve guardar o estado compartilhado do `AM2302`.

### Onde incluir

No mesmo arquivo `src/app/monitor.rs`, acima de `SystemMonitor`, adicione uma struct para o snapshot do sensor.

### Codigo sugerido

```rust
// Importa o tipo de erro do driver atual do AM2302.
use crate::drivers::am2302_capture::Am2302CaptureError;

// Snapshot simples com o ultimo estado conhecido do sensor.
#[derive(Debug, Clone, Copy)]
pub struct Am2302Snapshot {
    // Ultima temperatura valida em graus Celsius.
    pub temperature_c: Option<f32>,
    // Ultima umidade valida em %RH.
    pub humidity_rh: Option<f32>,
    // Momento da ultima atualizacao em ms desde o boot.
    pub last_update_ms: Option<u64>,
    // Ultimo erro retornado pelo driver.
    pub last_error: Option<Am2302CaptureError>,
}

impl Am2302Snapshot {
    // Estado inicial: ainda nao existe leitura valida nem erro conhecido.
    pub const fn new() -> Self {
        Self {
            temperature_c: None,
            humidity_rh: None,
            last_update_ms: None,
            last_error: None,
        }
    }

    // Atualiza o snapshot quando uma leitura termina com sucesso.
    pub fn update_success(&mut self, temperature_c: f32, humidity_rh: f32, now_ms: u64) {
        self.temperature_c = Some(temperature_c);
        self.humidity_rh = Some(humidity_rh);
        self.last_update_ms = Some(now_ms);
        self.last_error = None;
    }

    // Registra apenas o erro, preservando a ultima leitura valida.
    pub fn update_error(&mut self, error: Am2302CaptureError) {
        self.last_error = Some(error);
    }
}
```

### Depois disso

Dentro de `SystemMonitor`, adicione um novo campo:

```rust
// Snapshot mais recente do AM2302.
pub am2302: Am2302Snapshot,
```

E dentro de `SystemMonitor::new()` inicialize esse campo:

```rust
// Estado inicial do AM2302 antes da primeira leitura.
am2302: Am2302Snapshot::new(),
```

### Como deve ficar conceitualmente

No fim, `SystemMonitor` vai continuar guardando as metricas de tasks e tambem o snapshot do sensor.

---

## Passo 2: Atualizar a task do sensor em `src/main.rs`

### O que fazer

A task `am2302_task(...)` ja le o sensor.

Agora ela tambem precisa copiar esse resultado para o monitor compartilhado.

### Onde incluir

No arquivo `src/main.rs`, dentro da propria `am2302_task(...)`.

### Como esta hoje

Hoje a task faz algo conceitualmente assim:

```rust
match sensor.read(Irqs).await {
    Ok((temperature_c, humidity_rh)) => {
        info!("AM2302: temp={} C, humidity={} %RH", temperature_c, humidity_rh);
    }
    Err(error) => {
        error!("AM2302 error: {:?}", error);
    }
}
```

### Como deve ficar

Voce deve capturar `Instant::now().as_millis()` e atualizar `MONITOR` em cada ramo.

### Codigo sugerido

```rust
match sensor.read(Irqs).await {
    Ok((temperature_c, humidity_rh)) => {
        // Captura o instante em que a leitura valida terminou.
        let now_ms = Instant::now().as_millis() as u64;

        // Atualiza o snapshot compartilhado do AM2302.
        {
            let mut monitor = MONITOR.lock().await;
            monitor.am2302.update_success(temperature_c, humidity_rh, now_ms);
        }

        // Mantem o log RTT para observacao em bancada.
        info!("AM2302: temp={} C, humidity={} %RH", temperature_c, humidity_rh);
    }

    Err(error) => {
        // Registra o erro mais recente sem apagar a ultima leitura valida.
        {
            let mut monitor = MONITOR.lock().await;
            monitor.am2302.update_error(error);
        }

        // Mantem o log RTT para diagnostico.
        error!("AM2302 error: {:?}", error);
    }
}
```

### O que esta acontecendo aqui

- em caso de sucesso:
  - salva temperatura
  - salva umidade
  - salva o timestamp
  - limpa o erro anterior
- em caso de erro:
  - salva apenas o erro
  - mantem os ultimos valores validos

Isso permite que a shell mostre tanto a ultima medicao boa quanto o ultimo problema ocorrido.

---

## Passo 3: Adicionar `sensors` na tabela de comandos em `src/app/shell.rs`

### O que fazer

O comando precisa aparecer no `help`.

### Onde incluir

No array `COMMANDS` em `src/app/shell.rs`.

### Codigo sugerido

```rust
CommandEntry {
    // Nome digitado pelo usuario na shell.
    name: "sensors",
    // Descricao curta mostrada pelo comando `help`.
    help: "Mostra a ultima leitura dos sensores",
},
```

### Onde colocar

Pode inserir essa entrada perto de `status` ou depois de `tasks`.

O importante e ela ficar dentro do array `COMMANDS` junto com os demais comandos.

---

## Passo 4: Implementar o branch `"sensors"` em `execute_command(...)`

### O que fazer

O comando deve:

1. rejeitar argumentos extras
2. ler o snapshot do `AM2302` do `MONITOR`
3. montar a resposta textual em `ShellResponse`

### Onde incluir

No `match cmd.command` dentro de `execute_command(...)` em `src/app/shell.rs`.

### Estrutura recomendada

Inclua um branch novo como este:

```rust
"sensors" => {
    // Mantem o padrao dos outros comandos: nada de argumentos extras.
    if !cmd.args.is_empty() {
        let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
    } else {
        // Copia o snapshot e libera o lock antes de formatar a resposta.
        let am2302 = {
            let monitor = MONITOR.lock().await;
            monitor.am2302
        };

        // Cabecalho simples da resposta.
        let _ = out.push_str("Sensores:\r\n");

        match (am2302.temperature_c, am2302.humidity_rh, am2302.last_update_ms) {
            (Some(temperature_c), Some(humidity_rh), Some(last_update_ms)) => {
                // Mostra a ultima temperatura valida.
                let _ = out.push_str("AM2302 temp: ");
                let _ = write!(out, "{}", temperature_c);
                let _ = out.push_str(" C\r\n");

                // Mostra a ultima umidade valida.
                let _ = out.push_str("AM2302 humidity: ");
                let _ = write!(out, "{}", humidity_rh);
                let _ = out.push_str(" %RH\r\n");

                // Calcula ha quanto tempo essa leitura foi atualizada.
                let now_ms = Instant::now().as_millis() as u64;
                let age_ms = now_ms.saturating_sub(last_update_ms);
                let _ = out.push_str("Atualizado ha: ");
                let _ = write!(out, "{}", age_ms);
                let _ = out.push_str(" ms\r\n");
            }

            _ => {
                // Ainda nao houve leitura valida suficiente para mostrar dados.
                let _ = out.push_str("AM2302: sem leitura valida ainda\r\n");
            }
        }

        if let Some(error) = am2302.last_error {
            // Mostra o ultimo erro conhecido, se existir.
            let _ = out.push_str("Ultimo erro: ");
            let _ = write!(out, "{:?}", error);
            let _ = out.push_str("\r\n");
        }
    }
}
```

### O que observar

- o lock de `MONITOR` deve ser curto
- por isso, o ideal e copiar `monitor.am2302` para uma variavel local
- depois disso, a formatacao da resposta acontece sem segurar o mutex

Isso segue o mesmo estilo que o comando `tasks` ja usa.

---

## Passo 5: Verificar imports necessarios

### Em `src/app/monitor.rs`

Voce vai precisar do import do erro do driver:

```rust
// Tipo de erro do driver atual do AM2302.
use crate::drivers::am2302_capture::Am2302CaptureError;
```

### Em `src/app/shell.rs`

Provavelmente os imports principais ja existem, mas confirme que este continua presente:

```rust
// Permite montar strings formatadas com `write!`.
use core::fmt::Write;
```

E confirme que este tambem esta disponivel:

```rust
// Permite calcular a idade da ultima leitura do sensor.
use embassy_time::Instant;
```

### Em `src/main.rs`

Confirme que `Instant` continua importado:

```rust
// Permite registrar o instante de cada leitura valida.
use embassy_time::Instant;
```

---

## Passo 6: O que nao fazer

Para este passo continuar pequeno e correto, **nao faca** isto:

- nao chame `sensor.read()` dentro da shell
- nao crie outro driver do `AM2302` em `shell.rs`
- nao bloqueie a shell esperando resposta do sensor
- nao misture o protocolo do sensor com o parser de comandos
- nao tente incluir `HC-SR04` neste mesmo passo

O objetivo aqui e apenas expor na UART um estado que ja esta pronto.

---

## Passo 7: Checklist de implementacao

Antes de testar em hardware, revise se fez estes pontos:

1. criou `Am2302Snapshot` em `monitor.rs`
2. adicionou `am2302: Am2302Snapshot` em `SystemMonitor`
3. inicializou `am2302` em `SystemMonitor::new()`
4. atualizou `am2302_task(...)` para preencher o snapshot
5. adicionou `sensors` em `COMMANDS`
6. implementou o branch `"sensors"` em `execute_command(...)`

Se faltar qualquer um desses, o passo 5 ainda nao esta completo.

---

## Passo 8: Como validar

### Validacao de compilacao

Rode:

```text
cargo check
```

O esperado e compilar sem erro.

### Validacao em hardware

Rode:

```text
cargo run
```

Depois confira:

1. a placa inicializa normalmente
2. a shell imprime `Shell pronta. Digite 'help'.`
3. `help` lista `sensors`
4. `sensors` responde sem travar
5. a resposta bate com o que o RTT mostra

### Casos de teste uteis

Caso 1:

```text
help
```

Deve listar `sensors`.

Caso 2:

```text
sensors
```

Deve mostrar:

- temperatura e umidade, se ja houve leitura valida
- ou `AM2302: sem leitura valida ainda`

Caso 3:

```text
sensors extra
```

Deve responder:

```text
Erro: este comando nao aceita argumentos.
```

---

## Resumo final

O passo 5 fica correto quando:

- a task do `AM2302` continua lendo o sensor sozinha
- a leitura mais recente fica guardada em `MONITOR`
- o comando `sensors` so consulta esse estado compartilhado

Em outras palavras:

- o driver continua responsavel pela captura do protocolo
- a task continua responsavel pela atualizacao periodica
- a shell passa a ser apenas a camada de exibicao textual

Esse e o fechamento natural da Fase 5: o sensor deixa de ser visivel apenas pelo RTT e passa a ficar acessivel tambem pela UART de forma limpa e segura.
