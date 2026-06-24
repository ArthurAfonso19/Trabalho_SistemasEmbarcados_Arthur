## Echo na Shell UART

Este arquivo descreve como adicionar `echo` na shell UART atual.

Objetivo:

- fazer o terminal mostrar o que o usuario digita
- manter o envio assíncrono com `uart.write(...).await`
- tratar `Enter` e `Backspace` de forma utilizavel no PuTTY

---

## O que e `echo`

`echo` significa devolver pela UART os mesmos caracteres recebidos.

Exemplo:

1. o usuario digita `help`
2. a placa recebe `h`, `e`, `l`, `p`
3. a shell envia esses bytes de volta
4. o terminal mostra `help` na tela

Sem `echo`, o comando ainda pode funcionar, mas o usuario nao ve o que digitou se o `local echo` do terminal estiver desligado.

---

## Onde alterar

As mudancas ficam concentradas em:

- `src/app/shell.rs`

Nao e necessario alterar `src/main.rs` para esta melhoria.

### Como localizar cada ponto no arquivo

Para aplicar as mudancas, use estes marcos visuais dentro de `src/app/shell.rs`:

- procure a funcao `fn push_raw(&mut self, byte:u8) ->Result<(), ShellError>`
- procure a funcao `pub fn push_byte(&mut self, byte: u8) -> Result<(), ShellError>`
- procure o comentario `// Task da shell UART`
- procure o `loop` principal dentro de `pub async fn shell_taks(...)`

---

## Resumo das mudancas

Para implementar `echo`, a shell precisa de 3 ajustes:

1. adicionar uma funcao para remover o ultimo byte do buffer
2. adicionar uma funcao para ecoar o byte recebido
3. ajustar o loop principal da `shell_taks()`

---

## 1. Adicionar `pop_last()` em `ShellBuffer`

### Onde

Em `src/app/shell.rs`, dentro de `impl ShellBuffer`, logo depois de `push_raw()`.

### Como aplicar no arquivo

Faca o exemplo a seguir apos o fim da funcao `push_raw(...)`.

Em outras palavras:

- encontre este trecho no arquivo:

```rust
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
```

- depois desse trecho, adicione `pop_last(...)`
- antes da funcao `fn pop_raw(&mut self) -> Option<u8>`

### Antes

```rust
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
```

### Depois

```rust
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

fn pop_last(&mut self) -> Option<u8>
{
    // Se nao houver bytes acumulados, nao ha o que apagar.
    if self.len == 0
    {
        return None;
    }

    // Volta uma posicao na cauda do buffer circular.
    self.tail = (self.tail + RX_BUF_SIZE - 1) % RX_BUF_SIZE;
    let byte = self.buf[self.tail];
    self.len -= 1;
    Some(byte)
}
```

### Por que isso e necessario

Quando o usuario aperta `Backspace`, nao basta apagar visualmente na tela.
Tambem e preciso remover o ultimo byte armazenado no buffer da shell.

---

## 2. Adicionar uma funcao para ecoar a entrada

### Onde

Em `src/app/shell.rs`, antes de `shell_taks()`.

### Como aplicar no arquivo

Faca o exemplo a seguir apos o fim da funcao `execute_command(...)`.

Em outras palavras:

- encontre o final deste bloco:

```rust
pub fn execute_command(cmd: &ParsedCommand) -> ShellResponse
{
    let mut out = ShellResponse::new();

    match cmd.command{
        // ...
    }

    out
}
```

- depois desse fechamento, adicione `echo_input_byte(...)`
- antes do comentario `// Task da shell UART`

### Adicionar

```rust
async fn echo_input_byte(
    uart: &mut Uart<'static, embassy_stm32::mode::Async>,
    byte: u8,
) {
    match byte {
        b'\r' | b'\n' => {
            // Mostra uma quebra de linha correta no terminal serial.
            let _ = uart.write(b"\r\n").await;
        }
        0x08 | 0x7F => {
            // Move para tras, apaga o caractere na tela e volta de novo.
            let _ = uart.write(b"\x08 \x08").await;
        }
        _ => {
            // Ecoa o byte normal para o usuario ver o que digitou.
            let _ = uart.write(&[byte]).await;
        }
    }
}
```

### O que esta funcao faz

- ecoa caracteres normais
- converte `Enter` para `\r\n`
- trata `Backspace` e `Delete`

---

## 3. Ajustar o loop principal da `shell_taks()`

### Onde

Em `src/app/shell.rs`, dentro da task `shell_taks(...)`.

### Como aplicar no arquivo

Substitua o `loop` atual da `shell_taks()` pelo exemplo a seguir.

Em outras palavras:

- encontre dentro de `pub async fn shell_taks(...)` o bloco que comeca com:

```rust
loop 
{
    //Le um byte por vez da UART
    uart.read(&mut rx).await.unwrap();
```

- e vai ate o fechamento desse `loop`
- troque esse bloco inteiro pela versao nova com `echo`

### Antes

```rust
loop 
{
    //Le um byte por vez da UART
    uart.read(&mut rx).await.unwrap();

    //Alimenta o buffer da shell
    match shell.push_byte(rx[0])
    {
        Ok(()) => {}
        Err(ShellError::BufferFull) =>
        {
            let _ = uart.write(b"Error: linha muito longa.\r\n").await;
            shell.clear();
            continue;
        }
        Err(_) => 
        {
            let _ = uart.write(b"Erro de recepcao. \r\n").await;
            shell.clear();;
            continue;
        }
    } 

    //Quando a linha estiver pronta, extrai, faz parse e responde
    match shell.take_line()
    {
        Ok(Some(line)) => match parse_command(line.as_str())
        {
            Ok(Some(cmd)) => {
                let response = execute_command((&cmd));
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
```

### Depois

```rust
loop
{
    // Le um byte por vez da UART.
    uart.read(&mut rx).await.unwrap();
    let byte = rx[0];

    match byte {
        0x08 | 0x7F => {
            // Se o usuario apertou Backspace/Delete e ha algo no buffer,
            // apagamos do buffer e tambem da tela.
            if shell.pop_last().is_some() {
                echo_input_byte(&mut uart, byte).await;
            }
            continue;
        }
        b'\n' => {
            // Ignora LF sozinho para evitar eco duplo em terminais CRLF.
            continue;
        }
        b'\r' => {
            // Ecoa Enter como quebra de linha visual no terminal.
            echo_input_byte(&mut uart, byte).await;
        }
        _ => {
            // Ecoa o caractere normal antes de processar a entrada.
            echo_input_byte(&mut uart, byte).await;
        }
    }

    // Alimenta o buffer da shell com o byte recebido.
    match shell.push_byte(byte)
    {
        Ok(()) => {}
        Err(ShellError::BufferFull) =>
        {
            // A resposta comeca em nova linha para nao grudar no texto digitado.
            let _ = uart.write(b"\r\nErro: linha muito longa.\r\n").await;
            shell.clear();
            continue;
        }
        Err(_) =>
        {
            // Erro pequeno e direto para manter a shell limpa.
            let _ = uart.write(b"\r\nErro de recepcao.\r\n").await;
            shell.clear();
            continue;
        }
    }

    // Quando a linha estiver pronta, extrai, faz parse e responde.
    match shell.take_line()
    {
        Ok(Some(line)) => match parse_command(line.as_str())
        {
            Ok(Some(cmd)) => {
                let response = execute_command(&cmd);
                let _ = uart.write(response.as_bytes()).await;
            }
            Ok(None) => {
                // Linha vazia: nao responde nada.
            }
            Err(ShellError::TooManyArgs) => {
                let _ = uart.write(b"Erro: argumento demais.\r\n").await;
            }
            Err(_) => {
                let _ = uart.write(b"Erro ao interpretar comando.\r\n").await;
            }
        }
        Ok(None) => {
            // Linha ainda nao terminou.
        }
        Err(_) => {
            let _ = uart.write(b"Erro ao montar linha.\r\n").await;
        }
    }
}
```

### O que mudou

- caracteres normais passam a aparecer no terminal
- `Enter` quebra a linha corretamente
- `Backspace` apaga da tela e do buffer interno
- `LF` sozinho e ignorado para evitar dupla quebra de linha

---

## 4. Ajuste opcional em `push_byte()`

### Observacao

Com o novo loop, o `\n` ja e ignorado antes de chegar ao buffer.
Mesmo assim, voce pode simplificar `push_byte()` para deixar a regra mais clara.

### Como aplicar no arquivo

Se quiser fazer esse ajuste opcional, substitua a funcao `push_byte(...)` inteira.

Em outras palavras:

- encontre este bloco no arquivo:

```rust
pub fn push_byte(&mut self, byte: u8) -> Result<(), ShellError>
{
    if self.state != RxState::Receiving
    {
        return Ok(());
    }

    match byte {
        // ...
    }
}
```

- troque a funcao completa pela versao simplificada mostrada abaixo

### Antes

```rust
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
```

### Depois

```rust
pub fn push_byte(&mut self, byte: u8) -> Result<(), ShellError>
{
    if self.state != RxState::Receiving
    {
        return Ok(());
    }

    match byte {
        b'\r' => {
            // CR continua sendo o marcador de fim de linha.
            self.state = RxState::LineReady;
            Ok(())
        }
        b'\n' => {
            // LF sozinho e descartado pela shell.
            Ok(())
        }
        _ => self.push_raw(byte),
    }
}
```

### Vale a pena fazer?

Sim, mas e opcional.

Se voce quiser a mudanca mais minima possivel, pode deixar `push_byte()` como esta e aplicar apenas o novo loop com `echo`.

---

## 5. Resultado esperado no terminal

Se o usuario digitar:

```text
help
```

O terminal deve mostrar algo assim:

```text
help
Comandos disponíveis:
 - help: Lista os comandos disponíveis
 - status: Exibe um breve estado do sistema
```

Se o usuario digitar errado e usar `Backspace`, o ultimo caractere deve desaparecer visualmente e tambem deve ser removido do buffer.

---

## 6. Ordem recomendada de implementacao

1. adicionar `pop_last()`
2. adicionar `echo_input_byte(...)`
3. ajustar o loop da `shell_taks()`
4. opcionalmente simplificar `push_byte()`
5. testar com `local echo` desligado no PuTTY

---

## 7. Como testar

1. rodar `cargo check`
2. gravar com `cargo run`
3. abrir o PuTTY com `local echo` desligado
4. digitar `help`
5. digitar `status`
6. testar um comando invalido
7. testar `Backspace`

Resultado esperado:

- o que voce digita aparece no terminal
- `Enter` cria uma nova linha corretamente
- `Backspace` apaga de forma visual e funcional
- a shell continua respondendo normalmente

---

## 8. Observacao final

No seu codigo atual, a task se chama `shell_taks()`.

Isso nao impede o `echo`, mas o nome parece ter um pequeno erro de digitacao. Se quiser, pode renomear depois para `shell_task()` para deixar mais claro.
