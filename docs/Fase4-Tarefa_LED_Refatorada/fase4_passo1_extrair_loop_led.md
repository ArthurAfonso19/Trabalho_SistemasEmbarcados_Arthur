## Fase 4 - Passo 1: Extrair o loop do LED para `app/led_task.rs`

Este arquivo descreve o primeiro passo da Fase 4.

Objetivo deste passo:

- tirar o blink do LED de dentro de `src/main.rs`
- transformar esse blink em uma task formal do Embassy
- preparar a base para controle via shell nos proximos passos

Neste momento, o foco ainda nao e controlar o LED por comando.

O foco e apenas separar a responsabilidade do LED em um modulo proprio, sem mudar o comportamento visivel atual.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 4 define como primeiro passo:

1. extrair loop do LED para `app/led_task.rs`

O proprio ciclo de validacao da fase tambem deixa clara a ordem correta:

1. extrair o LED para task propria
2. confirmar que o blink continua funcionando
3. so depois adicionar controle on/off
4. testar via shell
5. adicionar ajuste de periodo por ultimo

Entao este passo ainda nao deve introduzir:

- `led on`
- `led off`
- `led period <ms>`
- estrutura completa de `LedControl`

Essas partes pertencem aos proximos passos da Fase 4.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/mod.rs`
- `src/app/led_task.rs`
- `src/main.rs`

No estado atual do projeto, essa e a menor mudanca coerente com o plano.

---

## Visao geral da ideia

Hoje o LED ainda pisca a partir de um loop localizado em `src/main.rs`.

Isso funciona, mas tem tres limitacoes para a Fase 4:

- o LED ainda nao e uma task formal separada
- o codigo de runtime continua concentrado demais em `main.rs`
- a shell ainda nao tem um ponto limpo para controlar o LED depois

Por isso, a ideia deste passo e simples:

1. mover a logica de blink para `src/app/led_task.rs`
2. transformar essa logica em uma task `#[embassy_executor::task]`
3. fazer `main.rs` apenas criar o pino e spawnar a task

O comportamento do LED deve continuar o mesmo que voce ja tinha antes da extracao.

---

## Escolha simples para este passo

Para este primeiro passo, a opcao mais segura e manter o comportamento atual do LED:

- blink continuo
- mesmo periodo atual
- mesma marcacao de monitoramento

Ou seja, neste momento a task extraida ainda pode receber apenas o proprio pino do LED e executar o loop normalmente.

Isso e melhor do que tentar introduzir `LedControl` ja agora, porque:

- reduz o risco de quebrar o blink
- separa a extracao da parte de controle
- respeita a ordem definida no plano

---

## 1. Criar o modulo `led_task` em `src/app/mod.rs`

### Onde aplicar

Faca isso em `src/app/mod.rs`.

### O que deve existir

Hoje esse arquivo ja reexporta os modulos da aplicacao, como `shell` e `monitor`.

Agora ele tambem precisa declarar o novo modulo do LED.

### Como fica

Adicione uma linha como esta:

```rust
pub mod led_task;
```

### Onde escrever exatamente

Procure em `src/app/mod.rs` as linhas atuais parecidas com:

```rust
pub mod shell;
pub mod monitor;
```

E adicione logo junto delas:

```rust
pub mod led_task;
```

### O que isso resolve

- permite que `main.rs` importe a task do LED
- cria um lugar proprio para a logica dessa tarefa

---

## 2. Criar o arquivo `src/app/led_task.rs`

### Onde aplicar

Crie um novo arquivo:

```text
src/app/led_task.rs
```

### Objetivo desse arquivo

Esse arquivo deve conter apenas a task do LED e a logica minima para o blink atual.

Neste passo 1, ele ainda nao precisa conter:

- `LedControl`
- parser de comandos
- leitura de argumentos da shell

### Estrutura minima sugerida

Uma forma simples e direta de comecar e esta:

```rust
use embassy_stm32::gpio::Output;
use embassy_stm32::time::Hertz;
use embassy_time::Timer;

// Usa a mesma funcao de monitoramento ja existente no projeto.
use crate::mark_led_execution;

#[embassy_executor::task]
pub async fn led_task(mut led: Output<'static>) {
    loop {
        // Marca a execucao da task antes de um novo ciclo de blink.
        mark_led_execution().await;

        // Mantem o comportamento atual: alterna o LED e espera o mesmo periodo.
        led.toggle();
        Timer::after_millis(1000).await;
    }
}
```

### Observacao importante

O tipo exato do pino do LED depende de como ele esta configurado hoje em `main.rs`.

Entao a ideia principal deste passo nao e copiar cegamente esse trecho, mas manter no novo modulo a mesma configuracao de `Output` e a mesma temporizacao que ja funcionavam antes.

### O que preservar da implementacao atual

Ao mover o codigo, preserve:

- o periodo atual do blink
- a forma atual de alternar o LED
- a chamada de monitoramento do LED

---

## 3. Localizar o loop atual do LED em `src/main.rs`

### O que procurar

Em `src/main.rs`, procure a parte final da execucao principal onde hoje o LED fica em loop.

No estado atual do projeto, essa logica tende a aparecer:

- perto do final do arquivo
- depois dos `spawn(...)` das outras tasks
- em um `loop { ... }` que nunca termina

### Qual parte deve sair de `main.rs`

Voce vai remover apenas a logica de blink que faz algo parecido com:

```rust
loop {
    // Marca a atividade do LED.
    mark_led_execution().await;

    // Alterna o estado do pino.
    led.toggle();

    // Espera o periodo atual do blink.
    Timer::after_millis(1000).await;
}
```

### O que nao deve sair de `main.rs`

Neste passo, `main.rs` ainda continua responsavel por:

- inicializar os perifericos
- criar o pino do LED
- spawnar as tasks

Ou seja, o que sai de `main.rs` e so o loop infinito do blink.

---

## 4. Importar a task do LED em `src/main.rs`

### Onde aplicar

No topo de `src/main.rs`, junto dos outros `use crate::app::...`.

### Como localizar o ponto exato

Hoje o arquivo ja possui imports como:

```rust
use crate::app::shell::shell_taks;
use crate::app::monitor::SystemMonitor;
```

Adicione tambem o import da nova task:

```rust
use crate::app::led_task::led_task;
```

### O que isso permite

Isso permite chamar `spawner.spawn(led_task(led))` a partir da `main()`.

---

## 5. Spawnar a task do LED em vez de rodar o loop em `main.rs`

### Onde aplicar

Ainda em `src/main.rs`, dentro da `main()`.

### Como localizar o ponto exato

Procure a regiao onde hoje a `main()`:

1. inicializa o pino do LED
2. faz `spawn(...)` das outras tasks
3. entra no loop final do blink

### O que fazer

Depois de criar o `led`, faca o spawn da task do LED, por exemplo:

```rust
unwrap!(spawner.spawn(led_task(led)));
```

E remova o loop final do blink de dentro da `main()`.

### Estrutura esperada

O final da `main()` deve evoluir conceitualmente de algo assim:

```rust
// Spawna outras tasks...
unwrap!(spawner.spawn(adc_task(adc, adc_pin)));
unwrap!(spawner.spawn(button_task(button)));
unwrap!(spawner.spawn(shell_taks(uart)));

// O LED ainda fica preso aqui dentro da main.
loop {
    mark_led_execution().await;
    led.toggle();
    Timer::after_millis(1000).await;
}
```

para algo assim:

```rust
// Spawna outras tasks...
unwrap!(spawner.spawn(adc_task(adc, adc_pin)));
unwrap!(spawner.spawn(button_task(button)));
unwrap!(spawner.spawn(shell_taks(uart)));

// Agora o LED tambem vira uma task formal.
unwrap!(spawner.spawn(led_task(led)));

// A main deixa de carregar o blink diretamente.
loop {
    Timer::after_secs(1).await;
}
```

### Observacao importante

Se a sua `main()` precisar permanecer viva sem fazer mais nada, deixe um loop final inofensivo como:

```rust
loop {
    Timer::after_secs(1).await;
}
```

O importante e que a logica do blink nao fique mais dentro dela.

---

## 6. Manter a marcacao de metricas do LED

### O que nao pode se perder

Na Fase 3, o LED ja passou a registrar metricas no monitor.

Ao extrair o loop para `src/app/led_task.rs`, essa marcacao precisa continuar existindo.

### Como garantir isso

Dentro da nova task, preserve a chamada:

```rust
mark_led_execution().await;
```

### Em que ponto chamar

A forma mais natural, neste passo, e chamar no inicio de cada ciclo do blink.

Exemplo:

```rust
loop {
    // Registra que a task do LED executou mais um ciclo.
    mark_led_execution().await;

    led.toggle();
    Timer::after_millis(1000).await;
}
```

### Por que isso importa

Assim o comando `tasks` continua mostrando a task `led` com metricas reais mesmo depois da extracao.

---

## 7. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- `LedControl`
- `led on`
- `led off`
- `led period <ms>`
- mudanca do botao para controlar o LED

Tudo isso entra nos proximos passos da Fase 4.

Neste passo 1, o sucesso e apenas:

- o LED virou uma task formal
- o blink continua funcionando
- o `tasks` continua mostrando metricas de `led`

---

## 8. Checklist de validacao

Antes de seguir para o passo 2, valide pelo menos isto:

1. `cargo check`
2. `cargo run`
3. o LED continua piscando
4. a shell continua iniciando
5. o comando `tasks` continua mostrando `led`

### Resultado esperado

Se este passo foi feito corretamente:

- o comportamento visual do LED continua igual
- a diferenca e apenas estrutural
- `main.rs` fica mais limpo
- a base para `led on/off/period` fica pronta

---

## Resumo deste passo

O passo 1 da Fase 4 nao e sobre novos comandos.

Ele e sobre reorganizar o firmware para que o LED deixe de ser um loop preso em `main.rs` e passe a ser uma task propria em `src/app/led_task.rs`.

Essa extracao deve ser feita com a menor mudanca possivel:

- mesmo blink
- mesma metrica
- mesma inicializacao do pino
- apenas melhor separacao de responsabilidades
