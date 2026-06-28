## Fase 4 - Passo 4: Comandos `led on`, `led off` e `led period <ms>`

Este arquivo descreve o quarto passo da Fase 4.

Objetivo deste passo:

- adicionar controle do LED pela shell UART
- usar o `LED_CONTROL` que ja existe no projeto
- permitir ligar, desligar e alterar o periodo do blink

Agora a Fase 4 deixa de ser apenas infraestrutura interna do LED.

Neste passo, o usuario finalmente ganha comandos visiveis para controlar a task do LED.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 4 define como quarto passo:

1. comandos:
   - `led on`
   - `led off`
   - `led period <ms>`

Entao o foco agora nao e criar mais estrutura de runtime.

O foco e expor o controle do LED pela shell, reaproveitando o `LedControl` e o `LED_CONTROL` criados nos passos anteriores.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/shell.rs`

Dependendo de como voce preferir organizar pequenas validacoes de periodo, tambem pode haver ajuste pequeno em:

- `src/app/led_task.rs`

Mas, no estado atual do projeto, a mudanca principal fica em `shell.rs`.

---

## Visao geral da ideia

No fim do passo 3, o projeto ja tem:

- uma task do LED em `src/app/led_task.rs`
- um `LedControl`
- um `LED_CONTROL` global protegido por `Mutex`

Ou seja, a parte de runtime ja esta pronta.

Agora a shell precisa conseguir:

1. reconhecer o comando `led`
2. analisar os argumentos recebidos
3. alterar `LED_CONTROL`
4. devolver uma resposta textual curta e clara ao usuario

---

## Escolha simples para este passo

Assim como foi feito com `tasks`, `uptime` e `mem`, a opcao mais simples e implementar `led` diretamente dentro de `execute_command()` em `src/app/shell.rs`.

Ou seja:

1. adicionar `led` em `COMMANDS`
2. adicionar um novo branch `"led" => { ... }` no `match cmd.command`
3. tratar os subcomandos dentro desse branch

Essa escolha e boa porque:

- reaproveita a shell atual
- exige pouca mudanca estrutural
- mantem a Fase 4 pequena e controlavel

---

## 1. Adicionar `led` na tabela `COMMANDS`

### Onde aplicar

Faca isso em `src/app/shell.rs`, dentro de `COMMANDS`.

### Como localizar o ponto exato

Procure o bloco atual parecido com:

```rust
pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        name: "help",
        help: "Lista os comandos disponíveis",
    },
    CommandEntry {
        name: "status",
        help: "Exibe um breve estado do sistema",
    },
    CommandEntry {
        name: "tasks",
        help: "Lista de tarefas com métricas",
    },
    CommandEntry {
        name: "uptime",
        help: "Mostra o tempo desde o boot",
    },
    CommandEntry {
        name: "mem",
        help: "Mostra o tamanho das seções de memória",
    },
];
```

Adicione uma nova entrada antes do fechamento `];`.

### Como fica

```rust
CommandEntry {
    name: "led",
    help: "Controla o LED: on, off, period <ms>",
},
```

### O que isso resolve

- `help` passa a listar `led`
- o comando fica descobrivel pelo usuario

---

## 2. Importar o controle global do LED em `shell.rs`

### Onde aplicar

Faca isso no topo de `src/app/shell.rs`, junto dos outros imports do crate.

### Como localizar o ponto exato

Hoje o arquivo ja importa algo como:

```rust
use crate::app::monitor::TaskMetrics;
use crate::MONITOR;
```

Agora a shell tambem precisa acessar `LED_CONTROL`.

### Como fica

```rust
use crate::app::monitor::TaskMetrics;
use crate::{LED_CONTROL, MONITOR};
```

Se preferir manter em linhas separadas, tambem funciona. O importante e que `LED_CONTROL` seja importado.

### O que isso permite

Isso permite que `execute_command()` faça `LED_CONTROL.lock().await` para ligar, desligar ou alterar o periodo.

---

## 3. Decidir como interpretar os argumentos do comando `led`

### Formatos suportados neste passo

O branch `led` deve aceitar exatamente estes formatos:

1. `led on`
2. `led off`
3. `led period <ms>`

### Formatos que devem ser rejeitados

Exemplos de entrada invalida:

1. `led`
2. `led period`
3. `led period abc`
4. `led on extra`
5. `led banana`

### Estrategia simples

Use `cmd.args` para decidir o que fazer.

Como `cmd.command` sera `"led"`, os argumentos entram em `cmd.args` assim:

- `led on` -> `args = ["on"]`
- `led off` -> `args = ["off"]`
- `led period 250` -> `args = ["period", "250"]`

---

## 4. Implementar o branch `"led" => { ... }` em `execute_command()`

### Onde aplicar

Faca isso em `src/app/shell.rs`, dentro de `pub async fn execute_command(...)`.

### Como localizar o ponto exato

Procure o `match cmd.command { ... }` onde hoje ja existem branches como:

- `"help" => { ... }`
- `"status" => { ... }`
- `"tasks" => { ... }`
- `"uptime" => { ... }`
- `"mem" => { ... }`

Adicione o branch `"led" => { ... }` antes do `_ => { ... }`.

### Como localizar o ponto exato no arquivo

No estado atual do projeto, `execute_command()` ja possui algo conceitualmente assim:

```rust
match cmd.command {
    "help" => {
        // ...
    }

    "status" => {
        // ...
    }

    "tasks" => {
        // ...
    }

    "uptime" => {
        // ...
    }

    "mem" => {
        // ...
    }

    "clean" => {
        // ...
    }

    _ => {
        // ...
    }
}
```

Entao o novo bloco do LED deve entrar logo antes do caso final `_ => { ... }`.

### O que este item faz na pratica

Neste item, voce nao cria uma funcao nova.

Voce tambem nao precisa mudar o parser da shell.

Voce apenas adiciona mais um branch dentro do `match cmd.command`, porque:

- `cmd.command` continua sendo `"led"`
- os detalhes `on`, `off` e `period <ms>` vao vir em `cmd.args`

Ou seja:

```text
led on
```

vira, conceitualmente:

```text
cmd.command = "led"
cmd.args = ["on"]
```

e:

```text
led period 250
```

vira, conceitualmente:

```text
cmd.command = "led"
cmd.args = ["period", "250"]
```

### Estrutura recomendada

Uma forma simples e clara e usar `match cmd.args.as_slice()`:

```rust
"led" => {
    // Decide o comportamento com base apenas nos argumentos apos o comando `led`.
    match cmd.args.as_slice() {
        ["on"] => {
            // Liga o LED de forma deterministica.
            let mut control = LED_CONTROL.lock().await;
            control.set_enabled(true);
            let _ = out.push_str("LED ligado.\r\n");
        }

        ["off"] => {
            // Desliga o LED de forma deterministica.
            let mut control = LED_CONTROL.lock().await;
            control.set_enabled(false);
            let _ = out.push_str("LED desligado.\r\n");
        }

        ["period", value] => {
            // Encaminha para a validacao e ajuste do novo periodo.
            // O tratamento detalhado entra no item 5.
        }

        _ => {
            // Qualquer outro formato cai em erro de uso.
            let _ = out.push_str("Uso: led on | led off | led period <ms>\r\n");
        }
    }
}
```

### Como implementar por partes sem se perder

Se quiser seguir a ordem mais segura, implemente assim:

1. adicione primeiro o branch `"led" => { ... }`
2. dentro dele, escreva primeiro apenas o `match cmd.args.as_slice()`
3. implemente o caso `["on"]`
4. implemente o caso `["off"]`
5. adicione o caso `["period", value]`
6. por ultimo, adicione o `_ => { ... }` com a mensagem de uso

### Estrutura minima inicial antes de preencher tudo

Se preferir montar o esqueleto primeiro, comente assim:

```rust
"led" => {
    // Analisa apenas os argumentos depois de `led`.
    match cmd.args.as_slice() {
        ["on"] => {
            // Aqui entra a logica para ligar o LED.
        }

        ["off"] => {
            // Aqui entra a logica para desligar o LED.
        }

        ["period", value] => {
            // Aqui entra a validacao do novo periodo em ms.
        }

        _ => {
            // Se o formato nao for reconhecido, responde com a forma correta de uso.
            let _ = out.push_str("Uso: led on | led off | led period <ms>\r\n");
        }
    }
}
```

### O que nao fazer neste item

Para manter o codigo claro:

- nao use `toggle()` para implementar `led on`
- nao use `toggle()` para implementar `led off`
- nao tente tratar `led`, `led on`, `led off` e `led period` em varios `if` separados fora do branch `"led"`

O mais limpo aqui e:

1. `cmd.command == "led"`
2. `match cmd.args.as_slice()`

### Observacao importante

No estado atual do `LedControl`, o passo 2 documentou `toggle`, `set_period`, `is_enabled` e `period_ms()`.

Mas para `led on` e `led off`, a forma mais limpa e previsivel e ter tambem um metodo explicito:

```rust
pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
}
```

Se esse metodo ainda nao existir em `src/app/led_task.rs`, adicione-o antes de implementar `led on/off`.

Usar `toggle()` aqui nao e a melhor escolha, porque:

- `led on` precisa sempre ligar
- `led off` precisa sempre desligar
- `toggle()` mudaria o estado dependendo do estado anterior

---

## 5. Implementar `led period <ms>`

### Onde aplicar

Ainda dentro do mesmo branch `"led"` em `execute_command()`.

### O que fazer

Quando os argumentos forem `[`"period"`, value]`, a shell deve:

1. tentar converter `value` para `u32`
2. validar se o valor e aceitavel
3. gravar o novo periodo em `LED_CONTROL`
4. responder com uma mensagem curta

### Exemplo sugerido

```rust
["period", value] => {
    match value.parse::<u32>() {
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
```

### Por que validar `>= 2`

No passo 3, a task do LED usa:

```rust
Timer::after_millis((period_ms / 2) as u64).await;
```

Se `period_ms` for muito pequeno, especialmente `0` ou `1`, a metade pode virar `0`, o que gera um comportamento ruim ou pouco util.

Entao uma validacao minima como `period_ms >= 2` ja evita esse problema sem complicar o projeto.

---

## 6. Ajuste pequeno necessario em `LedControl`

### Onde aplicar

Se ainda nao existir, faca isso em `src/app/led_task.rs`.

### Metodo recomendado

Adicione este metodo pequeno:

```rust
pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
}
```

### Onde escrever exatamente

Procure o `impl LedControl { ... }`, onde hoje ja existem metodos como:

- `toggle()`
- `set_period()`
- `is_enabled()`
- `period_ms()`

E adicione `set_enabled(...)` junto deles.

### O que isso resolve

- `led on` fica deterministico
- `led off` fica deterministico
- evita usar `toggle()` de forma inadequada

---

## 7. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- `sensors`
- `rt`
- botao alterando o estado do LED
- modos mais avancados como `blink once`, `pulse`, `pattern`

Neste passo 4, o sucesso e apenas:

- `help` lista `led`
- `led on` funciona
- `led off` funciona
- `led period <ms>` funciona
- a task do LED reage a essas mudancas

---

## 8. Checklist de validacao

Antes de considerar a Fase 4 concluida, valide pelo menos isto:

1. `cargo check`
2. `cargo run`
3. `help` lista `led`
4. `led off` apaga o LED
5. `led on` faz o blink voltar
6. `led period 200` acelera o blink
7. `led period 1000` volta para um ritmo mais lento
8. `led period 1` retorna erro coerente
9. `led banana` retorna mensagem de uso
10. `tasks` continua mostrando metricas de `led`

### Resultado esperado

Se este passo foi feito corretamente:

- o LED continua sendo uma task formal
- a shell controla o estado do LED
- o periodo pode ser ajustado em runtime
- o comportamento continua observavel no comando `tasks`

---

## Resumo deste passo

O passo 4 da Fase 4 e o momento em que o controle do LED finalmente chega a shell.

Para isso, a implementacao mais simples e coerente com o projeto atual e:

- adicionar `led` em `COMMANDS`
- importar `LED_CONTROL` em `shell.rs`
- criar um branch `"led" => { ... }` em `execute_command()`
- tratar:
  - `led on`
  - `led off`
  - `led period <ms>`
- adicionar `set_enabled(...)` em `LedControl` se ainda nao existir

Com isso, a Fase 4 passa a entregar exatamente o que o plano prometeu:

- LED como tarefa formal
- controle via shell
- metricas registradas
