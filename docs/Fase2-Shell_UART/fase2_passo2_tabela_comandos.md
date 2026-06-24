# Fase 2 - Passo 2 da Shell UART

Este arquivo mostra como desenvolver o Passo 2 da Fase 2.

Escopo deste passo:

- criar a tabela de comandos
- implementar `help`
- implementar `status`
- criar uma funcao que recebe um comando parseado e devolve uma resposta em texto

Importante: neste passo, a shell ainda nao precisa escrever diretamente na UART. A ideia e preparar a logica de comandos antes da integracao com a task serial.

---

## Arquivo onde tudo sera escrito

Todos os codigos deste passo entram em:

`src/app/shell.rs`

---

## Estrategia deste passo

Voce ja tem no Passo 1:

- `ShellBuffer`
- `RxState`
- `ShellError`
- `ParsedCommand`
- `parse_command()` ou `parse_comand()`

Agora, no Passo 2, vamos adicionar:

1. uma tabela de comandos
2. um tipo de resposta textual
3. uma funcao `execute_command()`

---

## 1. Adicionar o tipo de resposta da shell

### Onde escrever

Em `src/app/shell.rs`, logo abaixo destas linhas:

```rust
pub const RX_BUF_SIZE: usize = 64;
pub const MAX_ARGS: usize = 8;
```

adicione:

```rust
pub const RESPONSE_SIZE: usize = 256;

/// Resposta textual gerada pela shell.
///
/// Neste passo ainda nao escrevemos direto na UART.
/// Em vez disso, montamos a resposta em memoria e devolvemos para a camada chamadora.
pub type ShellResponse = String<RESPONSE_SIZE>;
```

### Para que isso serve

- define o tamanho maximo da resposta da shell
- permite montar respostas para `help` e `status`
- prepara a integracao futura com a UART

---

## 2. Adicionar a estrutura de entrada da tabela de comandos

### Onde escrever

Em `src/app/shell.rs`, logo abaixo da struct `ParsedCommand`, adicione:

```rust
/// Entrada simples da tabela de comandos.
pub struct CommandEntry {
    pub name: &'static str,
    pub help: &'static str,
}
```

### Para que isso serve

Cada comando da shell passa a ter:

- nome do comando
- descricao curta para o `help`

---

## 3. Adicionar a tabela de comandos

### Onde escrever

Ainda em `src/app/shell.rs`, logo abaixo da struct `CommandEntry`, adicione:

```rust
/// Tabela minima de comandos da shell nesta fase.
pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        name: "help",
        help: "Lista os comandos disponiveis",
    },
    CommandEntry {
        name: "status",
        help: "Exibe um breve estado do sistema",
    },
];
```

### Para que isso serve

Essa tabela sera usada para:

- listar comandos no `help`
- manter os comandos centralizados
- facilitar a expansao futura

---

## 4. Criar a funcao de execucao dos comandos

### Onde escrever

Em `src/app/shell.rs`, depois da funcao de parser (`parse_command()` ou `parse_comand()`), adicione:

```rust
/// Executa um comando ja parseado e devolve a resposta textual.
pub fn execute_command(cmd: &ParsedCommand) -> ShellResponse {
    let mut out = ShellResponse::new();

    match cmd.command {
        "help" => {
            let _ = out.push_str("Comandos disponiveis:\r\n");

            for entry in COMMANDS {
                let _ = out.push_str(" - ");
                let _ = out.push_str(entry.name);
                let _ = out.push_str(": ");
                let _ = out.push_str(entry.help);
                let _ = out.push_str("\r\n");
            }
        }
        "status" => {
            // Nesta fase, o status ainda e simples e fixo.
            // Mais para frente ele pode mostrar uptime, tarefas e memoria.
            let _ = out.push_str("Sistema ativo\r\n");
            let _ = out.push_str("UART: ok\r\n");
            let _ = out.push_str("Shell: inicial\r\n");
        }
        _ => {
            let _ = out.push_str("Comando desconhecido. Digite 'help'.\r\n");
        }
    }

    out
}
```

### Para que isso serve

Essa funcao recebe um `ParsedCommand` e decide:

- se e `help`
- se e `status`
- se e um comando desconhecido

Ela devolve uma `ShellResponse`, que depois sera enviada para a UART.

---

## 5. Nome da funcao de parser

Se no seu arquivo atual a funcao ainda estiver com este nome:

```rust
parse_comand
```

o ideal e renomear para:

```rust
parse_command
```

### Onde corrigir

Substitua:

```rust
pub fn parse_comand<'a>(line: &'a str) -> Result<Option<ParsedCommand<'a>>, ShellError>
```

por:

```rust
pub fn parse_command<'a>(line: &'a str) -> Result<Option<ParsedCommand<'a>>, ShellError>
```

### Por que corrigir

- evita erro de nomenclatura
- deixa o nome consistente com o restante do projeto
- melhora a leitura

---

## 6. Como o `src/app/shell.rs` ficara logicamente

Depois deste passo, o arquivo tera esta organizacao:

1. imports
2. constantes
3. `ShellResponse`
4. enums (`RxState`, `ShellError`)
5. structs (`ParsedCommand`, `CommandEntry`, `ShellBuffer`)
6. tabela `COMMANDS`
7. implementacao do buffer circular
8. parser de comando
9. executor de comandos

---

## 7. Exemplo de funcionamento

### Entrada

```text
help
```

### Saida gerada por `execute_command()`

```text
Comandos disponiveis:
 - help: Lista os comandos disponiveis
 - status: Exibe um breve estado do sistema
```

### Entrada

```text
status
```

### Saida gerada por `execute_command()`

```text
Sistema ativo
UART: ok
Shell: inicial
```

### Entrada

```text
foo
```

### Saida

```text
Comando desconhecido. Digite 'help'.
```

---

## 8. O que ainda nao faz parte deste passo

Ainda nao entra aqui:

- integrar com `main.rs`
- substituir a `uart_task` atual
- escrever a resposta na UART real
- criar uma `shell_task`
- usar metricas reais no `status`

Esses itens ficam para os proximos passos.

---

## 9. Resumo direto

Neste passo, em `src/app/shell.rs`, voce deve:

1. adicionar `ShellResponse`
2. adicionar `CommandEntry`
3. adicionar `COMMANDS`
4. adicionar `execute_command()`
5. corrigir `parse_comand` para `parse_command`, se necessario

Com isso, a shell ja passa a ter uma base minima de comandos antes da integracao com a UART.
