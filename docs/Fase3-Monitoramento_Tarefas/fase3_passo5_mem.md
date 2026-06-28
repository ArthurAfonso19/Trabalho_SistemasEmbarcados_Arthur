## Fase 3 - Passo 5: Comando `mem`

Este arquivo descreve o quinto passo da Fase 3.

Objetivo deste passo:

- adicionar o comando `mem` na shell UART
- expor o tamanho das secoes principais do firmware
- usar simbolos do linker para calcular esses tamanhos em runtime

Neste ponto, a Fase 3 fecha o conjunto de comandos de monitoramento planejados:

- `tasks`
- `uptime`
- `mem`

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 3 define como quinto passo:

1. comando `mem`
2. expor tamanho das secoes `.bss`, `.data`, `.text`
3. via simbolos do linker (consultar `memory.x`)

Entao o foco agora nao e criar mais monitoramento de tasks.

O foco e mostrar informacoes estaticas do layout de memoria do firmware.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/shell.rs`
- `src/main.rs`

Dependendo de como voce preferir organizar, tambem pode colocar uma funcao auxiliar em:

- `src/app/monitor.rs`

Mas, para manter esta etapa simples, a melhor opcao e deixar tudo em `shell.rs` e `main.rs`.

---

## Visao geral da ideia

O comando `mem` nao depende do `MONITOR` das tasks.

Ele depende de enderecos definidos pelo linker para marcar o inicio e o fim das secoes carregadas no binario.

Com esses simbolos, a ideia e calcular:

```text
tamanho = endereco_final - endereco_inicial
```

e entao montar uma resposta textual simples na shell, por exemplo:

```text
Memoria do firmware:
- .text: 74216 bytes
- .data: 968 bytes
- .bss: 4120 bytes
```

---

## Escolha simples para este passo

Para esta etapa, o melhor e manter o comando `mem` dentro de `execute_command()` em `src/app/shell.rs`, do mesmo jeito que `tasks` e `uptime`.

Ou seja:

1. adicionar `mem` em `COMMANDS`
2. adicionar o branch `"mem" => { ... }` em `execute_command()`
3. criar uma forma de ler os simbolos do linker

Essa abordagem e boa porque:

- reaproveita a shell atual
- exige pouca mudanca estrutural
- fecha a Fase 3 com o menor numero de arquivos novos

---

## 1. Entender de onde vem os simbolos

### O que existe hoje no projeto

O projeto ja usa linker script via `build.rs`:

```rust
println!("cargo:rustc-link-arg-bins=-Tlink.x");
```

Entao o firmware ja depende do `link.x` padrao do ecossistema embarcado.

Além disso, o projeto tambem tem `memory.x`, que adiciona secoes customizadas como:

- `.data2`
- `.ccmdata`

No arquivo `memory.x`, voce ja pode ver simbolos como:

```rust
__sdata2
__edata2
__sidata2
__sccmdata
__eccmdata
```

Esses simbolos ja sao usados em `src/main.rs` no `#[pre_init]`.

### O que isso significa para o passo 5

Para o comando `mem`, voce nao precisa inventar um mecanismo novo.

Voce vai seguir a mesma ideia:

- declarar simbolos externos
- ler seus enderecos
- calcular o tamanho entre inicio e fim

### Importante sobre `.text`, `.data` e `.bss`

O `memory.x` atual mostra bem as secoes customizadas, mas o plano do passo 5 quer especificamente:

- `.text`
- `.data`
- `.bss`

Essas secoes principais normalmente ja sao definidas pelo `link.x` usado no build.

Entao, neste passo, a consulta a `memory.x` serve mais para entender a estrategia geral do projeto com simbolos de linker do que para encontrar todos os simbolos exatos dessas tres secoes no proprio arquivo.

---

## 2. Onde implementar exatamente

Este passo se divide em dois lugares:

1. `src/main.rs`
2. `src/app/shell.rs`

### Em `src/main.rs`

Aqui fica a declaracao dos simbolos externos do linker, porque esse arquivo ja e o ponto central do firmware e ja lida com simbolos de memoria no `#[pre_init]`.

### Em qual ponto de `src/main.rs`

Procure a area perto do topo do arquivo, onde hoje ficam:

- `type AsyncI2c = ...`
- `type AccelDevice = ...`
- `const ADDRESS: u8 = 0x53;`
- `const WHOAMI: u8 = 0;`
- `pub(crate) static MONITOR: ...`

No estado atual do projeto, esse bloco aparece antes de `bind_interrupts!(struct Irqs { ... })`.

E exatamente nessa regiao que voce deve escrever os simbolos do linker e a funcao auxiliar do passo 5.

### Em `src/app/shell.rs`

Aqui entra o comando `mem`:

- na tabela `COMMANDS`
- no `match cmd.command` da `execute_command()`

### Em qual ponto de `src/app/shell.rs`

Voce vai mexer em dois lugares separados:

1. na constante `COMMANDS`, perto do topo do arquivo
2. dentro de `pub async fn execute_command(...)`

No estado atual do projeto, `COMMANDS` ja tem:

- `help`
- `status`
- `tasks`
- `uptime`

Entao o `mem` entra junto deles.

Ja dentro de `execute_command()`, o novo comando entra no mesmo `match` onde hoje ja existem os branches:

- `"help" => { ... }`
- `"status" => { ... }`
- `"tasks" => { ... }`
- `"uptime" => { ... }`

---

## 3. Declarar os simbolos do linker

### Onde aplicar

Faca isso em `src/main.rs`, perto de outras declaracoes globais.

### Como localizar o ponto exato

Procure este trecho atual:

```rust
const ADDRESS: u8 = 0x53;
const WHOAMI: u8 = 0;

pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

O ponto mais simples e escrever o bloco dos simbolos entre as constantes e o `MONITOR`.

### Como fica a estrutura

Fica assim:

```rust
const ADDRESS: u8 = 0x53;
const WHOAMI: u8 = 0;

unsafe extern "C" {
    static __stext: u8;
    static __etext: u8;
    static __sdata: u8;
    static __edata: u8;
    static __sbss: u8;
    static __ebss: u8;
}

pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

### O que fazer aqui

Neste item, voce nao precisa remover nada.

Voce apenas adiciona o bloco `unsafe extern "C" { ... }` nesse ponto do arquivo.

### Forma comum em Rust embarcado

Uma forma simples e declarar simbolos externos assim:

```rust
unsafe extern "C" {
    // Inicio e fim da secao .text no binario.
    static __stext: u8;
    static __etext: u8;

    // Inicio e fim da secao .data em RAM.
    static __sdata: u8;
    static __edata: u8;

    // Inicio e fim da secao .bss em RAM.
    static __sbss: u8;
    static __ebss: u8;
}
```

### O que isso faz

Essas declaracoes nao criam variaveis novas.

Elas apenas dizem ao compilador Rust que esses simbolos existem no binario final e serao resolvidos pelo linker.

---

## 4. Criar uma funcao auxiliar para calcular tamanhos

### Onde aplicar

Ainda em `src/main.rs`, crie uma funcao pequena para evitar repeticao.

### Como localizar o ponto exato

Depois de adicionar os simbolos externos, ainda em `src/main.rs`, procure a regiao logo antes de `fn stm32_config() -> Config { ... }`.

No estado atual do projeto, essa funcao `stm32_config()` aparece logo depois do bloco `#[pre_init]` e de alguns comentarios.

Para manter as funcoes utilitarias de memoria perto das declaracoes globais, a melhor escolha aqui e colocar `section_size(...)` antes de `stm32_config()`.

### Como fica no arquivo

Uma organizacao simples seria:

```rust
unsafe extern "C" {
    // simbolos...
}

pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());

fn section_size(start: *const u8, end: *const u8) -> usize {
    (end as usize).saturating_sub(start as usize)
}

pub(crate) fn firmware_memory_sizes() -> (usize, usize, usize) {
    // ...
}

fn stm32_config() -> Config {
    // ...
}
```

Exemplo:

```rust
fn section_size(start: *const u8, end: *const u8) -> usize {
    // Calcula o tamanho bruto da secao em bytes.
    (end as usize).saturating_sub(start as usize)
}
```

### Por que isso ajuda

- evita repetir a mesma conta tres vezes
- deixa o comando `mem` mais legivel
- combina com o estilo simples do projeto

---

## 5. Expor uma funcao pequena para a shell ler os tamanhos

### Por que isso e necessario

O comando `mem` vai ser implementado em `src/app/shell.rs`, mas os simbolos do linker estao mais naturais em `src/main.rs`.

Entao a shell precisa de um jeito simples de obter esses valores.

### Opcao direta e pequena

Em `src/main.rs`, crie uma funcao visivel dentro do crate:

### Onde escrever exatamente

Escreva essa funcao no mesmo arquivo `src/main.rs`, logo abaixo de `section_size(...)`.

Ou seja:

1. primeiro os simbolos externos
2. depois `section_size(...)`
3. depois `firmware_memory_sizes()`
4. depois o restante normal do arquivo

### Como localizar sem duvida

Se quiser seguir por referencia visual, deixe a funcao antes de:

```rust
fn stm32_config() -> Config {
```

```rust
pub(crate) fn firmware_memory_sizes() -> (usize, usize, usize) {
    unsafe {
        // Le os enderecos dos simbolos do linker e converte para tamanhos.
        let text_size = section_size(core::ptr::addr_of!(__stext), core::ptr::addr_of!(__etext));
        let data_size = section_size(core::ptr::addr_of!(__sdata), core::ptr::addr_of!(__edata));
        let bss_size = section_size(core::ptr::addr_of!(__sbss), core::ptr::addr_of!(__ebss));

        (text_size, data_size, bss_size)
    }
}
```

### O que essa funcao devolve

Na ordem:

1. tamanho de `.text`
2. tamanho de `.data`
3. tamanho de `.bss`

Isso deixa a shell desacoplada dos detalhes do linker.

---

## 6. Adicionar `mem` na tabela `COMMANDS`

### Onde aplicar

Faca isso em `src/app/shell.rs`, junto de `help`, `status`, `tasks` e `uptime`.

### Como localizar o ponto exato

Procure este bloco atual em `src/app/shell.rs`:

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
];
```

Adicione o `mem` dentro dessa lista, antes do fechamento `];`.

### Como fica

```rust
CommandEntry {
    name: "uptime",
    help: "Mostra o tempo desde o boot",
},

CommandEntry {
    name: "mem",
    help: "Mostra o tamanho das secoes de memoria",
},
```

### Exemplo

```rust
CommandEntry {
    name: "mem",
    help: "Mostra o tamanho das secoes de memoria",
},
```

Depois disso, o `help` ja passara a listar o novo comando.

---

## 7. Implementar o comando `mem`

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

Assim como no passo 4, o comando entra como mais um ramo no `match cmd.command`.

### Como localizar o ponto exato

Hoje, em `src/app/shell.rs`, a funcao tem um `match` parecido com este:

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

    _ => {
        // ...
    }
}
```

O branch `"mem" => { ... }` deve ser colocado antes do `_ => { ... }`.

Essa e a posicao mais natural, porque `_` precisa continuar sendo o ultimo caso do `match`.

### Como fica a estrutura

O trecho deve passar a ficar assim:

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
        // Aqui entra a leitura de crate::firmware_memory_sizes().
    }

    _ => {
        // ...
    }
}
```

### O que alterar e o que nao alterar

Neste item, voce nao precisa reescrever os comandos antigos.

Voce so adiciona um novo branch `"mem"` no `match` atual.

Nao precisa mexer em:

- `help`
- `status`
- `tasks`
- `uptime`
- `shell_taks()`

### O que o branch `"mem"` deve fazer

Dentro dele, voce deve:

1. rejeitar argumentos extras, como os outros comandos simples
2. chamar a funcao que devolve os tamanhos
3. montar a resposta textual

### Exemplo sugerido

```rust
"mem" => {
    if !cmd.args.is_empty() {
        // Mantem o comportamento consistente com help, status, tasks e uptime.
        let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
    } else {
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
```

### Como escrever sem se perder

Se quiser seguir exatamente a ordem mais segura, faca assim:

1. abra `src/app/shell.rs`
2. localize `pub async fn execute_command(...)`
3. entre no `match cmd.command`
4. encontre o branch `"uptime" => { ... }`
5. logo abaixo dele, adicione o branch `"mem" => { ... }`
6. mantenha `_ => { ... }` por ultimo

---

## 8. Cuidados importantes nesta etapa

### Confirmar os simbolos corretos do linker

O maior cuidado deste passo e garantir que os nomes usados nas declaracoes externas realmente existam no link final.

Os candidatos mais comuns sao:

- `__stext` e `__etext`
- `__sdata` e `__edata`
- `__sbss` e `__ebss`

Mas isso precisa bater com os simbolos fornecidos pelo linker do projeto.

Por isso, este passo deve ser sempre fechado com `cargo check`.

Se algum nome estiver incorreto, o erro mais provavel aparecera no processo de link ou em referencias indefinidas.

### Nao misturar `.data` com `.data2`

O projeto tem secoes customizadas em `memory.x`:

- `.data2`
- `.ccmdata`

Essas secoes sao importantes para o firmware, mas o plano do passo 5 pede especificamente:

- `.text`
- `.data`
- `.bss`

Entao, para concluir este passo, nao e necessario exibir `.data2` nem `.ccmdata` no comando `mem`.

Se quiser, isso pode virar uma expansao futura, mas nao deve complicar este passo agora.

### Evitar logica desnecessaria na shell

O comando `mem` deve ser simples.

Ele nao precisa:

- acessar `MONITOR`
- usar `Mutex`
- fazer `await` extra alem do proprio fluxo async da shell
- calcular porcentagens ou uso dinamico de heap

O objetivo aqui e apenas mostrar tamanhos estaticos de secoes do firmware.

---

## 9. O que deve existir ao final do passo 5

Ao concluir esta etapa, o projeto deve ter:

- `mem` listado em `COMMANDS`
- `help` mostrando `mem`
- simbolos de linker declarados para as secoes necessarias
- uma funcao que calcula os tamanhos de `.text`, `.data` e `.bss`
- um branch `"mem"` em `execute_command()`

Com isso, os entregaveis da Fase 3 passam a ficar completos:

- `tasks`
- `uptime`
- `mem`

---

## 10. Resultado esperado deste passo

Depois da implementacao, voce deve conseguir abrir a shell serial e testar:

```text
help
mem
```

Comportamento esperado:

1. `help` lista `mem`
2. `mem` mostra os tamanhos de `.text`, `.data` e `.bss`
3. `mem abc` responde erro de argumentos

Exemplo de saida esperada:

```text
Memoria do firmware:
- .text: 74216 bytes
- .data: 968 bytes
- .bss: 4120 bytes
```

Os numeros exatos vao variar conforme o binario atual.

---

## 11. Resumo direto

Neste passo, voce deve:

1. declarar simbolos externos do linker em `src/main.rs`
2. criar uma funcao para calcular os tamanhos das secoes
3. expor essa funcao para a shell
4. adicionar `mem` em `COMMANDS`
5. implementar o branch `"mem"` em `execute_command()`
6. validar com `cargo check`

Depois disso, a Fase 3 fica concluida.
