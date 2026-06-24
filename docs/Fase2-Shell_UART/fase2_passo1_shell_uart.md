# Fase 2 - Passo 1 da Shell UART

Este arquivo mostra como implementar o primeiro passo da Fase 2.

Escopo deste passo:

- buffer de entrada circular de 64 bytes
- estado de recepcao
- parser que separa comando e argumentos

Importante: neste passo, a shell ainda nao precisa estar integrada a `main.rs` nem a uma task UART nova. A ideia aqui e montar a base da logica de recepcao e parsing.

---

## Arquivo a criar

Criar o arquivo:

`src/app/shell.rs`

---

## Codigo para `src/app/shell.rs`

```rust
use heapless::{String, Vec};

pub const RX_BUF_SIZE: usize = 64;
pub const MAX_ARGS: usize = 8;

/// Estado atual da recepcao da linha vinda pela UART.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxState {
    Receiving,
    LineReady,
    Overflow,
}

/// Erros simples da camada de shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellError {
    BufferFull,
    TooManyArgs,
}

/// Resultado do parser.
///
/// Os argumentos apontam para a propria linha parseada.
pub struct ParsedCommand<'a> {
    pub command: &'a str,
    pub args: Vec<&'a str, MAX_ARGS>,
}

/// Buffer circular simples para receber bytes da UART.
pub struct ShellBuffer {
    buf: [u8; RX_BUF_SIZE],
    head: usize,
    tail: usize,
    len: usize,
    state: RxState,
}

impl ShellBuffer {
    pub const fn new() -> Self {
        Self {
            buf: [0; RX_BUF_SIZE],
            head: 0,
            tail: 0,
            len: 0,
            state: RxState::Receiving,
        }
    }

    pub fn state(&self) -> RxState {
        self.state
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
        self.state = RxState::Receiving;
    }

    fn push_raw(&mut self, byte: u8) -> Result<(), ShellError> {
        if self.len == RX_BUF_SIZE {
            self.state = RxState::Overflow;
            return Err(ShellError::BufferFull);
        }

        self.buf[self.tail] = byte;
        self.tail = (self.tail + 1) % RX_BUF_SIZE;
        self.len += 1;
        Ok(())
    }

    fn pop_raw(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }

        let byte = self.buf[self.head];
        self.head = (self.head + 1) % RX_BUF_SIZE;
        self.len -= 1;
        Some(byte)
    }

    /// Recebe um byte da UART.
    ///
    /// Regras:
    /// - `\r` e ignorado
    /// - `\n` marca fim de linha
    /// - outros bytes entram no buffer circular
    pub fn push_byte(&mut self, byte: u8) -> Result<(), ShellError> {
        if self.state != RxState::Receiving {
            return Ok(());
        }

        match byte {
            b'\r' => Ok(()),
            b'\n' => {
                self.state = RxState::LineReady;
                Ok(())
            }
            _ => self.push_raw(byte),
        }
    }

    /// Extrai a linha pronta do buffer circular para uma `String<64>`.
    pub fn take_line(&mut self) -> Result<Option<String<RX_BUF_SIZE>>, ShellError> {
        if self.state != RxState::LineReady {
            return Ok(None);
        }

        let mut line: String<RX_BUF_SIZE> = String::new();

        while let Some(byte) = self.pop_raw() {
            if line.push(byte as char).is_err() {
                self.clear();
                return Err(ShellError::BufferFull);
            }
        }

        self.state = RxState::Receiving;
        Ok(Some(line))
    }
}

/// Faz o parse de uma linha no formato:
/// `comando arg1 arg2 arg3`
pub fn parse_command<'a>(line: &'a str) -> Result<Option<ParsedCommand<'a>>, ShellError> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut parts = trimmed.split_ascii_whitespace();

    let command = match parts.next() {
        Some(cmd) => cmd,
        None => return Ok(None),
    };

    let mut args: Vec<&str, MAX_ARGS> = Vec::new();

    for part in parts {
        if args.push(part).is_err() {
            return Err(ShellError::TooManyArgs);
        }
    }

    Ok(Some(ParsedCommand { command, args }))
}
```

---

## O que esse codigo resolve

### 1. Buffer circular de 64 bytes

Isso fica dentro de `ShellBuffer`:

```rust
buf: [u8; RX_BUF_SIZE],
head: usize,
tail: usize,
len: usize,
```

Funcao:

- armazenar os bytes recebidos pela UART
- evitar depender de heap
- permitir receber a linha byte a byte

### 2. Estado de recepcao

Isso fica no enum:

```rust
pub enum RxState {
    Receiving,
    LineReady,
    Overflow,
}
```

Funcao:

- `Receiving`: ainda recebendo a linha
- `LineReady`: recebeu `\n`, linha pronta para parse
- `Overflow`: a linha passou de 64 bytes

### 3. Parser de comando e argumentos

Isso fica na funcao:

```rust
pub fn parse_command<'a>(line: &'a str) -> Result<Option<ParsedCommand<'a>>, ShellError>
```

Exemplos:

Entrada:

```text
help
```

Saida:

- `command = "help"`
- `args = []`

Entrada:

```text
led period 500
```

Saida:

- `command = "led"`
- `args[0] = "period"`
- `args[1] = "500"`

---

## Como isso sera usado depois

No proximo passo, a task UART da shell vai fazer algo conceitualmente assim:

```rust
let mut shell = ShellBuffer::new();

shell.push_byte(byte)?;

if let Some(line) = shell.take_line()? {
    if let Some(cmd) = parse_command(line.as_str())? {
        // despachar comando
    }
}
```

Ou seja:

- `push_byte()` recebe byte por byte
- `take_line()` monta a linha quando chegar `\n`
- `parse_command()` separa comando e argumentos

---

## O que ainda nao faz parte deste passo

Ainda nao entra aqui:

- integracao com `main.rs`
- criacao de `src/app/mod.rs`
- nova `shell_task`
- tabela de comandos reais
- comando `help`
- escrita de resposta pela UART

Tudo isso fica para os proximos passos da Fase 2.

---

## Resumo

Neste passo, voce cria a base da shell em `src/app/shell.rs` com:

- recepcao de bytes
- montagem de linha
- controle de estado
- parser de comando

Isso ja prepara o projeto para, no passo seguinte, substituir o eco UART por um console de verdade.
