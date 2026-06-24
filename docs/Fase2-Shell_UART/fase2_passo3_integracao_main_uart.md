# Fase 2 - Passo 3 da Shell UART

Este arquivo mostra como desenvolver o Passo 3 da Fase 2.

Escopo deste passo:

- integrar a shell com a UART real
- criar `shell_task` recebendo a UART
- substituir o eco serial atual por leitura de linha + resposta de comandos
- conectar isso em `main.rs`

Importante: neste passo, o objetivo e fazer `help` e `status` funcionarem pela UART real.

---

## Arquivos que serao alterados

Neste passo, voce vai mexer em:

- `src/app/shell.rs`
- `src/main.rs`

---

## Visao geral do que muda

Hoje o projeto tem uma task assim em `src/main.rs`:

- `uart_task(...)`
- ela le 1 byte
- faz eco do byte de volta

Depois deste passo, o comportamento passa a ser:

- a UART continua sendo lida byte a byte
- os bytes vao para `ShellBuffer`
- quando chega `\n`, a linha e montada
- a linha passa por `parse_command()`
- o resultado passa por `execute_command()`
- a resposta textual e escrita na UART

---

## 1. Adicionar `shell_task` em `src/app/shell.rs`

### Onde escrever

Em `src/app/shell.rs`, depois da funcao `execute_command(...)`, adicione o codigo abaixo.

### Codigo a adicionar

```rust
use defmt::info;
use embassy_stm32::usart::Uart;

/// Task da shell UART.
///
/// Responsabilidades:
/// - ler bytes da UART
/// - alimentar o buffer circular
/// - detectar linha completa
/// - fazer parse do comando
/// - executar o comando
/// - escrever a resposta de volta na UART
#[embassy_executor::task]
pub async fn shell_task(mut uart: Uart<'static, embassy_stm32::mode::Async>) {
    let mut shell = ShellBuffer::new();
    let mut rx = [0u8; 1];

    info!("Shell UART iniciada");
    let _ = uart.write(b"Shell pronta. Digite 'help'.\r\n").await;

    loop {
        // Le um byte por vez da UART.
        uart.read(&mut rx).await.unwrap();

        // Alimenta o buffer da shell.
        match shell.push_byte(rx[0]) {
            Ok(()) => {}
            Err(ShellError::BufferFull) => {
                let _ = uart.write(b"Erro: linha muito longa.\r\n").await;
                shell.clear();
                continue;
            }
            Err(_) => {
                let _ = uart.write(b"Erro de recepcao.\r\n").await;
                shell.clear();
                continue;
            }
        }

        // Quando a linha estiver pronta, extrai, faz parse e responde.
        match shell.take_line() {
            Ok(Some(line)) => match parse_command(line.as_str()) {
                Ok(Some(cmd)) => {
                    let response = execute_command(&cmd);
                    let _ = uart.write(response.as_bytes()).await;
                }
                Ok(None) => {
                    // Linha vazia: nao responde nada.
                }
                Err(ShellError::TooManyArgs) => {
                    let _ = uart.write(b"Erro: argumentos demais.\r\n").await;
                }
                Err(_) => {
                    let _ = uart.write(b"Erro ao interpretar comando.\r\n").await;
                }
            },
            Ok(None) => {
                // Linha ainda nao terminou.
            }
            Err(_) => {
                let _ = uart.write(b"Erro ao montar linha.\r\n").await;
                shell.clear();
            }
        }
    }
}
```

### O que esse codigo faz

- cria `ShellBuffer`
- le 1 byte da UART por vez
- usa `push_byte()` para alimentar o buffer
- usa `take_line()` para detectar linha completa
- usa `parse_command()` para separar comando e argumentos
- usa `execute_command()` para montar a resposta
- escreve a resposta na UART com `uart.write(...)`

---

## 2. Ajustar imports em `src/app/shell.rs`

### Onde alterar

No topo de `src/app/shell.rs`.

### Como provavelmente esta hoje

Algo perto disto:

```rust
use heapless::{String, Vec};
```

### Como deve ficar

```rust
use defmt::info;
use embassy_stm32::usart::Uart;
use heapless::{String, Vec};
```

### Observacao

Se ainda existir este import no seu arquivo:

```rust
use crate::app::shell::RxState::Receiving;
```

ele pode ser removido, porque nao e necessario.

---

## 3. Substituir o uso de `uart_task` por `shell_task` em `src/main.rs`

### 3.1. Importar a task da shell

### Onde alterar

No topo de `src/main.rs`, logo depois do `mod app;` ou junto dos imports.

### Adicionar

```rust
use crate::app::shell::shell_task;
```

---

## 4. Trocar o spawn da UART em `main.rs`

Hoje voce provavelmente tem o setup da UART seguido do spawn da task antiga.

### Como estava antes

Trecho conceitual em `src/main.rs`:

```rust
let lpuart1 = init_uart(
    p.LPUART1,
    p.PA3,
    p.PA2,
    p.DMA1_CH1,
    p.DMA1_CH2,
);

spawner.spawn(unwrap!(uart_task(lpuart1)));
```

### Como deve ficar agora

```rust
let lpuart1 = init_uart(
    p.LPUART1,
    p.PA3,
    p.PA2,
    p.DMA1_CH1,
    p.DMA1_CH2,
);

spawner.spawn(unwrap!(shell_task(lpuart1)));
```

### O que mudou

- a UART continua sendo criada da mesma forma
- quem passa a consumir a UART agora e a `shell_task`
- a task antiga de eco deixa de ser usada

---

## 5. O que fazer com `uart_task(...)`

Voce tem duas opcoes:

### Opcao A - manter por enquanto, mas sem usar

Pode deixar a funcao no arquivo e apenas parar de spawnar.

Vantagem:

- mudanca menor
- mais facil voltar atras se precisar

Desvantagem:

- vai gerar warning de funcao nao usada

### Opcao B - remover depois

Depois que a shell estiver validada no hardware, voce pode apagar `uart_task(...)`.

### Recomendacao

Para este passo, prefira a Opcao A.

---

## 6. Trecho exato de alteracao em `src/main.rs`

### 6.1. Adicionar import

### Antes

```rust
mod app;
```

### Depois

```rust
mod app;

use crate::app::shell::shell_task;
```

---

### 6.2. Alterar o spawn da UART

### Antes

```rust
spawner.spawn(unwrap!(uart_task(lpuart1)));
```

### Depois

```rust
spawner.spawn(unwrap!(shell_task(lpuart1)));
```

---

## 7. Como testar este passo

Depois de implementar:

1. rodar `cargo check`
2. se compilar, rodar `cargo run`
3. abrir o terminal serial ligado a `LPUART1`
4. verificar a mensagem inicial:

```text
Shell pronta. Digite 'help'.
```

5. testar:

```text
help
```

Saida esperada:

```text
Comandos disponiveis:
 - help: Lista os comandos disponiveis
 - status: Exibe um breve estado do sistema
```

6. testar:

```text
status
```

Saida esperada:

```text
Sistema ativo
UART: ok
Shell: inicial
```

7. testar um comando invalido:

```text
abc
```

Saida esperada:

```text
Comando desconhecido. Digite 'help'.
```

---

## 8. O que este passo conclui

Ao final deste passo, voce tera:

- shell conectada a UART real
- leitura byte a byte funcionando
- parse de linha funcionando no fluxo real
- `help` funcionando pela serial
- `status` funcionando pela serial

---

## 9. O que ainda fica para depois

Ainda nao entra aqui:

- parser mais robusto
- historico de comandos
- buffer circular de saida
- comandos `tasks`, `uptime`, `mem`, `rt`, `sensors`
- integracao com metricas reais de sistema

Esses itens entram nas proximas fases/passos.

---

## 10. Resumo direto

Neste passo, voce deve:

1. adicionar `shell_task(...)` em `src/app/shell.rs`
2. ajustar imports de `shell.rs`
3. importar `shell_task` em `src/main.rs`
4. trocar:

```rust
uart_task(lpuart1)
```

por:

```rust
shell_task(lpuart1)
```

Com isso, a shell deixa de ser apenas uma logica em memoria e passa a funcionar pela UART real.
