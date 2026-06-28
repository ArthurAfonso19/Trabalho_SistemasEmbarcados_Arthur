## Fase 3 - Passo 3: Marcacao em `adc_task`, `button_task` e `led`

Este arquivo descreve o terceiro passo da Fase 3.

Objetivo deste passo:

- conectar o monitoramento ja criado ao fluxo real das tasks
- registrar execucoes de `adc_task` e `button_task`
- registrar tambem a atividade do LED usando a implementacao atual do projeto

Neste passo, o foco ainda nao e mostrar metricas na shell.
O foco e fazer as tasks atualizarem `SystemMonitor` em tempo de execucao.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 3 define como terceiro passo:

1. adicionar marcacao em `led_task`, `button_task`, `adc_task`

No codigo atual, isso significa:

- `adc_task` ja existe em `src/main.rs`
- `button_task` ja existe em `src/main.rs`
- o LED ainda nao foi extraido para `led_task`
- hoje o comportamento do LED esta em `run_led_loop`

Entao, nesta etapa, a adaptacao correta e:

- instrumentar `adc_task`
- instrumentar `button_task`
- instrumentar `run_led_loop` como representacao atual da task de LED

O nome da metrica continua sendo `led`, porque isso bate com `SystemMonitor` e com o plano da fase.

---

## Arquivos alterados neste passo

Neste passo, a implementacao fica concentrada em:

- `src/main.rs`

Em principio, nao e necessario alterar:

- `src/app/mod.rs`
- `src/app/shell.rs`
- `src/app/monitor.rs`

Isso assume que `TaskMetrics` e `SystemMonitor` ja foram criados nos passos 1 e 2.

---

## Visao geral da ideia

Agora que existe um container central chamado `SystemMonitor`, o proximo passo e disponibilizar esse estado para as tasks.

Cada task vai fazer algo como:

```rust
// Le o tempo atual desde o boot, em milissegundos.
let now_ms = Instant::now().as_millis() as u64;

// Entra na secao critica async para acessar o monitor compartilhado.
let mut monitor = MONITOR.lock().await;
// Atualiza as metricas da task ADC com o timestamp atual.
monitor.adc.mark_execution(now_ms);
```

O ponto importante e:

- a task registra sua propria execucao
- o lock do monitor deve durar pouco
- a task nao deve manter o monitor bloqueado durante `await`

---

## Escolha simples para este passo

Para manter esta etapa pequena e segura, a abordagem mais direta e usar um monitor compartilhado estatico em `src/main.rs`.

Uma forma adequada para este projeto e:

```rust
// Traz a struct que agrupa todas as metricas do sistema.
use crate::app::monitor::SystemMonitor;
// Define o tipo de lock de baixo nivel usado pelo Mutex no firmware.
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
// Mutex async usado para compartilhar o monitor entre tasks.
use embassy_sync::mutex::Mutex;
// Instant fornece o tempo atual desde o boot.
use embassy_time::Instant;

// Instancia unica e global do monitoramento.
static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    // O monitor ja nasce com adc, button e led inicializados.
    Mutex::new(SystemMonitor::new());
```

Essa escolha e boa nesta fase porque:

- evita alocacao dinamica
- funciona bem em firmware `no_std`
- permite compartilhamento simples entre tasks async
- combina com a inicializacao `const fn new()` criada no passo 2

---

## 1. Importar o monitoramento em `src/main.rs`

### Onde aplicar

Faca isso no topo de `src/main.rs`, junto dos outros `use`.

### O que adicionar

Adicione os imports necessarios para:

- acessar `SystemMonitor`
- criar o `Mutex`
- ler o tempo atual em milissegundos

Exemplo:

```rust
// Importa o container central das metricas.
use crate::app::monitor::SystemMonitor;
// Tipo de mutex adequado para secao critica no microcontrolador.
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
// Permite acesso seguro ao estado compartilhado entre tasks async.
use embassy_sync::mutex::Mutex;
// Instant le o tempo atual e Timer continua sendo usado nos delays.
use embassy_time::{Instant, Timer};
```

Se `Timer` ja estiver importado separadamente, basta ajustar para nao duplicar import.

---

## 2. Criar o monitor compartilhado

### Onde aplicar

Ainda em `src/main.rs`, declare um `static` em escopo de modulo, antes das tasks.

### Exemplo sugerido

```rust
// Estado global compartilhado entre as tasks monitoradas.
static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    // Inicializa todas as metricas com valores zerados.
    Mutex::new(SystemMonitor::new());
```

### O que isso faz

- cria uma unica instancia global de `SystemMonitor`
- permite acesso concorrente seguro entre tasks async
- garante que todas as metricas comecem zeradas e com nomes definidos

---

## 3. Criar uma funcao pequena para marcar execucao

### Por que vale a pena

Como tres pontos diferentes vao registrar metricas, uma funcao auxiliar pequena ajuda a evitar repeticao.

Exemplo:

```rust
async fn mark_adc_execution() {
    // Captura o instante atual em milissegundos.
    let now_ms = Instant::now().as_millis() as u64;
    // Abre o monitor compartilhado por um periodo curto.
    let mut monitor = MONITOR.lock().await;
    // Atualiza somente a metrica da task ADC.
    monitor.adc.mark_execution(now_ms);
}

async fn mark_button_execution() {
    // Captura o instante atual em milissegundos.
    let now_ms = Instant::now().as_millis() as u64;
    // Abre o monitor compartilhado por um periodo curto.
    let mut monitor = MONITOR.lock().await;
    // Atualiza somente a metrica da task do botao.
    monitor.button.mark_execution(now_ms);
}

async fn mark_led_execution() {
    // Captura o instante atual em milissegundos.
    let now_ms = Instant::now().as_millis() as u64;
    // Abre o monitor compartilhado por um periodo curto.
    let mut monitor = MONITOR.lock().await;
    // Atualiza somente a metrica do LED.
    monitor.led.mark_execution(now_ms);
}
```

Essa abordagem e simples e deixa explicito qual metrica cada task atualiza.

Se voce preferir, tambem pode escrever o bloco de marcacao diretamente dentro de cada loop, sem criar helper.

---

## 4. Instrumentar `adc_task`

### Onde aplicar

Faca isso dentro do loop de `adc_task` em `src/main.rs`.

### Trecho atual

Hoje a task faz a leitura e depois espera 500 ms:

```rust
loop {
    let _measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);
    embassy_time::Timer::after_millis(500).await;
}
```

### Ajuste sugerido

Logo depois da leitura, marque a execucao:

```rust
loop {
    // Faz uma nova aquisicao do canal analogico.
    let _measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);
    // Registra que a task ADC executou neste ciclo.
    mark_adc_execution().await;
    // Mantem o periodo atual da task.
    Timer::after_millis(500).await;
}
```

### Por que marcar nesse ponto

- a leitura do ADC acabou de acontecer
- a task registra atividade real, nao apenas o fato de ter acordado
- o intervalo observado vai refletir o ritmo de aquisicao da task

---

## 5. Instrumentar `button_task`

### Onde aplicar

Faca isso dentro do loop de `button_task`.

### Escolha recomendada

Marque a execucao quando a task detectar o pressionamento.

Exemplo:

```rust
loop {
    // Espera o usuario pressionar o botao.
    button.wait_for_rising_edge().await;
    // Conta um novo evento de press no monitoramento.
    mark_button_execution().await;
    info!("Pressed!");

    // Espera o usuario soltar o botao antes de reiniciar o ciclo.
    button.wait_for_falling_edge().await;
    info!("Released!");
}
```

### Por que essa escolha e boa

- cada press do usuario gera uma marcacao
- a metrica passa a representar eventos reais do botao
- o contador fica facil de interpretar

Se voce quiser medir tanto subida quanto descida, isso tambem e possivel, mas aumenta o contador em dobro e pode deixar a leitura menos intuitiva nesta fase.

---

## 6. Instrumentar o LED atual em `run_led_loop`

### Importante

O plano cita `led_task`, mas o codigo atual ainda nao extraiu o LED para uma task formal.

Hoje o LED esta aqui:

```rust
async fn run_led_loop(...) -> ! {
    loop {
        // Liga o LED no inicio do ciclo visual.
        led.set_high();
        Timer::after_millis(500).await;

        // Desliga o LED para completar o blink.
        led.set_low();
        Timer::after_millis(500).await;
    }
}
```

### Como adaptar nesta fase

Instrumente `run_led_loop` e trate essa execucao como a metrica da task `led`.

Exemplo:

```rust
loop {
    // Marca o inicio de um novo ciclo do LED.
    led.set_high();
    // Registra a execucao associada a esse ciclo.
    mark_led_execution().await;
    // Mantem o LED aceso pelo periodo atual.
    Timer::after_millis(500).await;

    // Completa o blink desligando o LED.
    led.set_low();
    // Mantem o LED apagado antes do proximo ciclo.
    Timer::after_millis(500).await;
}
```

### Por que marcar na borda de subida

- cada ciclo do LED ganha um ponto unico de referencia
- evita contar duas vezes o mesmo periodo visual
- mantem a metrica facil de entender

Mais adiante, na Fase 4, quando o LED virar uma task formal, essa marcacao podera ser movida para `led_task` sem mudar a ideia do monitoramento.

---

## 7. O que exatamente deve existir ao final

Ao terminar este passo, o projeto deve ter:

- um `MONITOR` compartilhado acessivel em `src/main.rs`
- `adc_task` chamando `mark_execution()` para a metrica `adc`
- `button_task` chamando `mark_execution()` para a metrica `button`
- o loop atual do LED chamando `mark_execution()` para a metrica `led`

Ainda nao precisa existir neste passo:

- comando `tasks`
- comando `uptime`
- comando `mem`
- formatacao de saida na shell

---

## 8. Cuidados importantes nesta etapa

### Nao segurar o lock durante `await`

O padrao correto e:

```rust
// Captura o timestamp antes de entrar no lock.
let now_ms = Instant::now().as_millis() as u64;
// Abre o monitor apenas para atualizar a metrica.
let mut monitor = MONITOR.lock().await;
// Faz a marcacao rapidamente, sem awaits no meio.
monitor.adc.mark_execution(now_ms);
```

Depois disso, o guard deve sair de escopo logo em seguida.

Nao faca algo assim:

```rust
// Abre o monitor compartilhado.
let mut monitor = MONITOR.lock().await;
// Atualiza a metrica da task.
monitor.adc.mark_execution(now_ms);
// Errado: aqui o lock continuaria preso durante a espera async.
Timer::after_millis(500).await;
```

Isso manteria o recurso compartilhado bloqueado por tempo demais.

---

## 9. Resultado esperado deste passo

Ao final, o firmware ainda pode parecer igual externamente, porque ainda nao existe comando novo na shell.

Mas internamente, o sistema passa a registrar:

- quantas vezes o ADC executou
- quando o botao foi pressionado
- quantos ciclos do LED foram marcados
- intervalos minimos e maximos observados para essas atividades

Esse e o passo que transforma `TaskMetrics` e `SystemMonitor` em algo realmente alimentado pelo runtime.

---

## 10. Resumo direto

Neste passo, voce deve:

1. abrir `src/main.rs`
2. importar `SystemMonitor`, `Mutex` e `Instant`
3. criar um `static MONITOR`
4. marcar execucao em `adc_task`
5. marcar execucao em `button_task`
6. marcar execucao no loop atual do LED
7. manter o lock curto e sem atravessar `await`

Depois disso, a Fase 3 fica pronta para o passo 4, que e expor essas metricas no comando `tasks`.
