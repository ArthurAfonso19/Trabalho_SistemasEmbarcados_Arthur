## Fase 3 - Passo 4: Comandos `tasks` e `uptime`

Este arquivo descreve o quarto passo da Fase 3.

Objetivo deste passo:

- expor as metricas ja coletadas pela instrumentacao das tasks
- adicionar um comando `tasks` na shell UART
- adicionar um comando `uptime` para mostrar o tempo desde o boot

Agora a Fase 3 deixa de ser apenas infraestrutura interna.
Neste passo, o usuario finalmente ganha comandos novos visiveis na shell.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 3 define como quarto passo:

1. comandos novos:
2. `tasks` -> lista tarefas com metricas
3. `uptime` -> tempo desde o boot

Existe tambem um detalhe importante no proprio ciclo de validacao da fase:

1. testar o comando `tasks` antes de adicionar `mem` e `uptime`

Entao a ordem mais segura nesta etapa e:

1. implementar `tasks`
2. validar a compilacao
3. depois adicionar `uptime`

Mesmo assim, este arquivo ja documenta os dois porque os dois pertencem ao passo 4.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/shell.rs`
- `src/main.rs`

Dependendo de como voce quiser organizar o acesso ao monitor, tambem pode fazer um ajuste pequeno em:

- `src/app/monitor.rs`

No estado atual do projeto, a solucao mais direta e manter a mudanca principal em `main.rs` e `shell.rs`.

---

## Visao geral da ideia

No passo 3, o firmware passou a registrar metricas em um `MONITOR` compartilhado.

Agora, a shell precisa conseguir:

- ler esse estado compartilhado
- montar uma resposta textual
- enviar essa resposta de volta pela UART

Como a shell atual ja possui o fluxo:

1. ler comando
2. fazer parse
3. executar
4. escrever resposta

o passo 4 e basicamente expandir esse fluxo para suportar `tasks` e `uptime`.

---

## Escolha simples para este passo

No codigo atual, `execute_command()` em `src/app/shell.rs` ainda e sincrona:

```rust
// Hoje a shell recebe um comando parseado.
pub fn execute_command(cmd: &ParsedCommand) -> ShellResponse {
    // Monta uma resposta textual.
    // Ainda nao faz acesso async a estado compartilhado.
}
```

Para o comando `tasks`, isso deixa de ser suficiente, porque sera preciso ler o `MONITOR`, e esse acesso usa `lock().await`.

Por isso, a adaptacao mais direta neste passo e:

```rust
// A execucao dos comandos passa a ser async.
pub async fn execute_command(cmd: &ParsedCommand) -> ShellResponse {
    // Agora ela pode esperar lock do monitor quando necessario.
}
```

Essa escolha e boa porque:

- exige pouca mudanca estrutural
- reaproveita o fluxo atual da shell UART
- permite que so os comandos que precisam de estado compartilhado usem `await`

---

## 1. Tornar o monitor acessivel fora de `main.rs`

### Problema atual

Hoje o `MONITOR` esta declarado em `src/main.rs`:

```rust
// Instancia unica e global do monitoramento.
static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

Assim, a shell ainda nao consegue ler esse estado diretamente.

### Onde implementar exatamente

Faca essa mudanca no proprio `src/main.rs`.

Procure a declaracao global do monitor, perto do topo do arquivo, junto das constantes e tipos globais.

No estado atual do projeto, ela aparece assim:

```rust
// Cria unica instancia global de `SystemMonitor`.
// Permite acesso concorrente seguro entre tasks async.
// Garante que todas as metricas comecem zeradas e com nomes definidos.
static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

Essa e exatamente a linha que deve ser alterada.

### Ajuste minimo recomendado

Transforme o monitor em item visivel dentro do crate:

```rust
// Permite que outros modulos do projeto leiam o monitor compartilhado.
pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    // A inicializacao continua identica a do passo 3.
    Mutex::new(SystemMonitor::new());
```

### O que muda na pratica

Voce nao precisa mover o `MONITOR` para outro arquivo.

Voce nao precisa apagar a inicializacao atual.

Voce nao precisa criar outro monitor novo na shell.

O ajuste e apenas este:

```rust
// Antes: visivel so dentro de main.rs.
static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());

// Depois: visivel para outros modulos do mesmo crate.
pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

Ou seja:

- mantem o mesmo nome: `MONITOR`
- mantem o mesmo tipo
- mantem a mesma inicializacao
- so adiciona `pub(crate)` na frente

### Precisa retirar algo?

Nao.

Neste item, voce nao precisa remover nada do passo 3.

O `MONITOR` que ja existe continua sendo o mesmo.

O que voce faz e apenas ampliar a visibilidade dele para que `src/app/shell.rs` possa importar `crate::MONITOR`.

### Por que isso resolve

- `main.rs` continua usando o mesmo monitor
- `src/app/shell.rs` passa a conseguir importar `crate::MONITOR`
- evita mover a estrutura para outro arquivo antes de haver necessidade real

---

## 2. Adicionar `tasks` na tabela de comandos da shell

### Onde aplicar

Faca isso em `src/app/shell.rs`, dentro de `COMMANDS`.

### Antes

Hoje a tabela contem apenas:

```rust
// Tabela minima de comandos da shell nesta fase.
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

### Depois

Adicione `tasks` e `uptime`:

```rust
// Tabela de comandos expandida para monitoramento.
pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        name: "help",
        help: "Lista os comandos disponiveis",
    },
    CommandEntry {
        name: "status",
        help: "Exibe um breve estado do sistema",
    },
    CommandEntry {
        name: "tasks",
        help: "Lista tarefas com metricas",
    },
    CommandEntry {
        name: "uptime",
        help: "Mostra o tempo desde o boot",
    },
];
```

### O que isso prepara

- `help` passa a listar os novos comandos
- os nomes dos comandos ficam centralizados
- a shell ganha uma interface mais coerente para a Fase 3

---

## 3. Tornar `execute_command()` async

### Onde aplicar

Ainda em `src/app/shell.rs`, altere a assinatura da funcao.

### Antes

```rust
// Executa um comando ja parseado e devolve a resposta textual.
pub fn execute_command(cmd: &ParsedCommand) -> ShellResponse {
    let mut out = ShellResponse::new();
    // ...
}
```

### Depois

```rust
// Executa um comando ja parseado e devolve a resposta textual.
// Agora e async porque alguns comandos precisam ler estado compartilhado.
pub async fn execute_command(cmd: &ParsedCommand) -> ShellResponse {
    let mut out = ShellResponse::new();
    // ...
}
```

### Ajuste correspondente na task da shell

Como `shell_taks()` ja e async, a chamada tambem precisa mudar:

```rust
// Executa o comando e espera a resposta ser montada.
let response = execute_command(&cmd).await;
// Escreve a resposta na UART.
let _ = uart.write(response.as_bytes()).await;
```

---

## 4. Importar o que `tasks` e `uptime` vao precisar

### Onde aplicar

No topo de `src/app/shell.rs`.

### O que adicionar

```rust
// Permite acessar o monitor compartilhado criado em main.rs.
use crate::MONITOR;
// Instant permite calcular o tempo desde o boot para o comando uptime.
use embassy_time::Instant;
```

Se preferir, voce pode tambem importar `TaskMetrics` ou `SystemMonitor`, mas para esta etapa isso nao e obrigatorio.

---

## 5. Implementar o comando `tasks`

### Onde implementar exatamente

Faca isso em `src/app/shell.rs`, dentro da funcao:

```rust
pub async fn execute_command(cmd: &ParsedCommand<'_>) -> ShellResponse {
    let mut out = ShellResponse::new();

    match cmd.command {
        // ...
    }

    out
}
```

Ou seja, o comando `tasks` nao e implementado em `main.rs`.

Ele deve entrar como um novo ramo dentro do `match cmd.command` da `execute_command()`.

### Ponto exato do arquivo

No seu arquivo atual, existe um trecho parecido com este:

```rust
match cmd.command {
    "help" => {
        // ...
    }

    "status" => {
        // ...
    }

    _ => {
        // ...
    }
}
```

E exatamente ai que voce deve adicionar o novo caso `"tasks"`.

### Como fica a estrutura

O ideal e transformar esse trecho em algo assim:

```rust
match cmd.command {
    // Lista os comandos disponiveis.
    "help" => {
        // ...
    }

    // Mantem o status simples ja existente.
    "status" => {
        // ...
    }

    // Novo comando da Fase 3 para mostrar metricas das tasks.
    "tasks" => {
        // Aqui entra a leitura do MONITOR e a montagem da resposta.
    }

    // Novo comando da Fase 3 para mostrar tempo desde o boot.
    "uptime" => {
        // ...
    }

    // Continua tratando comando desconhecido.
    _ => {
        // ...
    }
}
```

### O que esse bloco deve fazer

Dentro do branch `"tasks"`, voce deve:

1. acessar `MONITOR.lock().await`
2. ler `monitor.adc`, `monitor.button` e `monitor.led`
3. escrever esses dados na `ShellResponse`

Entao o fluxo fica assim:

```rust
"tasks" => {
    // Le o estado compartilhado do monitoramento.
    let monitor = MONITOR.lock().await;

    // Monta a resposta textual que sera enviada pela UART.
    // Aqui voce escreve os dados de adc, button e led em `out`.
}
```

### O que nao precisa fazer aqui

Neste item, voce nao precisa:

- mexer no loop de `adc_task`
- mexer no loop de `button_task`
- mexer no `run_led_loop`
- criar outro monitor
- implementar esse texto em `main.rs`

Essas partes ja ficaram prontas no passo 3.

No passo 4 item 5, o trabalho e somente ensinar a shell a ler o monitor e responder ao comando digitado pelo usuario.

### O que o comando deve mostrar

O objetivo nao e fazer uma tabela sofisticada ainda.
O objetivo e devolver uma resposta legivel com:

- nome da task
- contador de execucoes
- timestamp da ultima execucao
- intervalo minimo
- intervalo maximo

### Estrategia simples

Dentro do branch `"tasks"`, leia o monitor e escreva um bloco por task.

Exemplo:

```rust
"tasks" => {
    // Entra no monitor compartilhado para ler as metricas atuais.
    let monitor = MONITOR.lock().await;

    // Cabecalho simples do comando.
    let _ = out.push_str("Tasks monitoradas:\r\n");

    // ADC.
    let _ = out.push_str("- ");
    let _ = out.push_str(monitor.adc.name);
    let _ = out.push_str(": count=");
    let _ = write!(out, "{}", monitor.adc.execution_count);
    let _ = out.push_str(", last_ms=");
    let _ = write!(out, "{:?}", monitor.adc.last_run_ms);
    let _ = out.push_str(", min_ms=");
    let _ = write!(out, "{:?}", monitor.adc.min_interval_ms);
    let _ = out.push_str(", max_ms=");
    let _ = write!(out, "{:?}\r\n", monitor.adc.max_interval_ms);

    // Button.
    let _ = out.push_str("- ");
    let _ = out.push_str(monitor.button.name);
    let _ = out.push_str(": count=");
    let _ = write!(out, "{}", monitor.button.execution_count);
    let _ = out.push_str(", last_ms=");
    let _ = write!(out, "{:?}", monitor.button.last_run_ms);
    let _ = out.push_str(", min_ms=");
    let _ = write!(out, "{:?}", monitor.button.min_interval_ms);
    let _ = out.push_str(", max_ms=");
    let _ = write!(out, "{:?}\r\n", monitor.button.max_interval_ms);

    // LED.
    let _ = out.push_str("- ");
    let _ = out.push_str(monitor.led.name);
    let _ = out.push_str(": count=");
    let _ = write!(out, "{}", monitor.led.execution_count);
    let _ = out.push_str(", last_ms=");
    let _ = write!(out, "{:?}", monitor.led.last_run_ms);
    let _ = out.push_str(", min_ms=");
    let _ = write!(out, "{:?}", monitor.led.min_interval_ms);
    let _ = out.push_str(", max_ms=");
    let _ = write!(out, "{:?}\r\n", monitor.led.max_interval_ms);
}
```

### Importante sobre `write!`

Como a resposta e um `heapless::String`, o jeito mais pratico de montar texto com numeros e usar `core::fmt::Write`.

No topo de `src/app/shell.rs`, adicione:

```rust
// Habilita o macro write! para montar respostas com numeros.
use core::fmt::Write;
```

### Por que essa forma e suficiente agora

- evita criar formatadores complicados cedo demais
- mostra todas as metricas relevantes
- deixa o comando `tasks` testavel imediatamente pela UART

---

## 6. Implementar o comando `uptime`

### Ideia

O tempo desde o boot pode ser obtido diretamente com `Instant::now()`.

### Exemplo simples

```rust
"uptime" => {
    // Le o tempo atual desde o boot.
    let uptime_ms = Instant::now().as_millis();

    // Monta uma resposta textual curta e direta.
    let _ = out.push_str("Uptime: ");
    let _ = write!(out, "{}", uptime_ms);
    let _ = out.push_str(" ms\r\n");
}
```

### Observacao

Nesta fase, mostrar o valor em milissegundos ja e suficiente.

Se quiser melhorar depois, voce pode formatar em segundos, minutos ou `hh:mm:ss`, mas isso nao e necessario para concluir o passo 4.

---

## 7. Estrutura sugerida do `match` em `execute_command()`

Depois da expansao, o `match` pode ficar assim:

```rust
match cmd.command {
    // Lista os comandos da shell.
    "help" => {
        // ...
    }

    // Mantem o status simples ja existente.
    "status" => {
        // ...
    }

    // Mostra as metricas das tasks monitoradas.
    "tasks" => {
        // ...
    }

    // Mostra o tempo desde o boot.
    "uptime" => {
        // ...
    }

    // Trata comando nao reconhecido.
    _ => {
        // ...
    }
}
```

Essa ordem ajuda porque deixa primeiro os comandos antigos, depois os novos de monitoramento.

---

## 8. Cuidados importantes nesta etapa

### Nao deixar a resposta crescer demais

Hoje `ShellResponse` usa um `String<256>`.

Com `tasks`, esse limite pode ficar apertado dependendo do texto final.

Isso acontece porque o comando `tasks` junta varias partes na mesma resposta:

- cabecalho do comando
- linha da task `adc`
- linha da task `button`
- linha da task `led`
- numeros convertidos para texto
- `\r\n` no fim de cada linha

Mesmo sem formatacao bonita em colunas, esse texto cresce rapido.

Por exemplo, uma saida como esta ja ocupa um espaco consideravel:

```text
Tasks monitoradas:
- adc: count=120, last_ms=Some(10450), min_ms=Some(500), max_ms=Some(503)
- button: count=4, last_ms=Some(9320), min_ms=Some(1200), max_ms=Some(4100)
- led: count=98, last_ms=Some(10380), min_ms=Some(1000), max_ms=Some(1002)
```

Se o buffer for pequeno demais, os `push_str(...)` e `write!(...)` podem falhar por falta de espaco.

Na pratica, isso pode causar:

- resposta truncada
- partes finais faltando
- perda de alguma linha da saida

### O que fazer se isso acontecer

A solucao mais simples nesta fase e aumentar o tamanho maximo da resposta.

Se a resposta com as tres tasks ficar grande demais, voce pode aumentar com seguranca:

```rust
// Aumenta o espaco da resposta para acomodar os comandos de monitoramento.
pub const RESPONSE_SIZE: usize = 512;
```

Essa mudanca continua simples e e razoavel para este passo.

### Onde ajustar isso

Faca essa alteracao em `src/app/shell.rs`, na declaracao:

```rust
pub const RESPONSE_SIZE: usize = 256;
```

Se necessario, troque para:

```rust
pub const RESPONSE_SIZE: usize = 512;
```

### Como saber se 256 ainda serve

Se voce montar o comando `tasks` e notar que precisa escrever muitos `let _ = out.push_str(...)` com textos longos, vale a pena ja subir para 512.

Como este projeto e embarcado e usa `heapless::String`, esse ajuste precisa ser intencional: o tamanho e fixo em compilacao.

Ou seja, diferente de uma `String` dinamica, aqui o buffer nao cresce sozinho.

### Nao manter o lock do monitor por mais tempo que o necessario

O comando `tasks` pode montar a resposta enquanto o lock estiver ativo, porque isso ainda e curto e simples.

Mas e importante entender o risco: enquanto a shell segura `MONITOR.lock().await`, outras partes do firmware que tambem quiserem acessar esse mesmo monitor terao que esperar.

Neste projeto, isso afeta principalmente:

- `mark_adc_execution()`
- `mark_button_execution()`
- `mark_led_execution()`

Essas funcoes tambem usam o mesmo `MONITOR`.

Entao, se a shell ficar muito tempo formatando texto com o lock aberto, as tasks podem ficar momentaneamente impedidas de atualizar as metricas.

Nao costuma ser um problema grave para esta fase, mas e uma boa pratica evitar isso desde ja.

### Forma simples e aceitavel

Para este passo, esta forma ainda pode ser aceita:

```rust
"tasks" => {
    // Abre o monitor compartilhado.
    let monitor = MONITOR.lock().await;

    // Usa diretamente monitor.adc, monitor.button e monitor.led
    // para montar a resposta textual.
}
```

Ela e aceitavel porque:

- o numero de tasks e pequeno
- a resposta ainda e curta
- a implementacao fica mais facil de entender

### Forma mais limpa

Se quiser fazer um pouco melhor, copie os dados primeiro e solte o lock logo em seguida.

Mas, se quiser deixar mais limpo, voce pode copiar os dados primeiro:

```rust
// Copia as metricas para variaveis locais e libera o lock cedo.
let (adc, button, led) = {
    let monitor = MONITOR.lock().await;
    (monitor.adc, monitor.button, monitor.led)
};
```

Depois disso, a formatacao usa so os valores copiados.

Como `TaskMetrics` ja e `Copy`, essa opcao funciona bem.

### Por que isso melhora

Nesse formato:

1. o lock e usado apenas para leitura rapida
2. a shell sai da secao critica cedo
3. a formatacao de texto acontece sem bloquear atualizacoes das tasks

Na pratica, a diferenca e esta:

```rust
// Menos ideal: formata texto enquanto ainda segura o lock.
let monitor = MONITOR.lock().await;
let _ = write!(out, "{}", monitor.adc.execution_count);
let _ = write!(out, "{:?}", monitor.button.last_run_ms);
let _ = write!(out, "{:?}", monitor.led.max_interval_ms);
```

```rust
// Melhor: copia os dados, libera o lock e so depois formata.
let (adc, button, led) = {
    let monitor = MONITOR.lock().await;
    (monitor.adc, monitor.button, monitor.led)
};

let _ = write!(out, "{}", adc.execution_count);
let _ = write!(out, "{:?}", button.last_run_ms);
let _ = write!(out, "{:?}", led.max_interval_ms);
```

### Regra pratica para este passo

Se quiser a implementacao mais simples possivel, pode montar `tasks` com o lock aberto e seguir em frente.

Se quiser uma versao um pouco melhor, copie `adc`, `button` e `led` para variaveis locais antes de formatar a resposta.

As duas abordagens funcionam, mas a segunda e mais organizada.

---

## 9. O que deve existir ao final do passo 4

Ao concluir esta etapa, o projeto deve ter:

- comando `tasks` listado no `help`
- comando `uptime` listado no `help`
- `execute_command()` capaz de responder `tasks`
- `execute_command()` capaz de responder `uptime`
- `shell_taks()` chamando `execute_command(...).await`

O que ainda nao faz parte deste passo:

- comando `mem`
- simbolos do linker para `.bss`, `.data` e `.text`
- refinar LED como task formal da Fase 4

---

## 10. Resultado esperado deste passo

Depois da implementacao, voce deve conseguir abrir a shell serial e testar:

```text
help
tasks
uptime
```

Comportamento esperado:

1. `help` lista `help`, `status`, `tasks` e `uptime`
2. `tasks` mostra metricas de `adc`, `button` e `led`
3. `uptime` mostra o tempo desde o boot em milissegundos

Esse passo fecha a primeira entrega realmente visivel do monitoramento.

---

## 11. Resumo direto

Neste passo, voce deve:

1. tornar `MONITOR` acessivel para a shell
2. adicionar `tasks` e `uptime` na tabela `COMMANDS`
3. transformar `execute_command()` em async
4. ajustar `shell_taks()` para usar `.await`
5. implementar o branch `tasks`
6. implementar o branch `uptime`
7. aumentar `RESPONSE_SIZE` se a resposta ficar grande demais

Depois disso, a Fase 3 fica pronta para o passo 5, que e o comando `mem`.
