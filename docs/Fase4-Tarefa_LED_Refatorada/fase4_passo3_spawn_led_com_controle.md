## Fase 4 - Passo 3: Spawnar `led_task` com `LedControl`

Este arquivo descreve o terceiro passo da Fase 4.

Objetivo deste passo:

- conectar a task do LED ao estado `LedControl`
- permitir que a task leia `enabled` e `period_ms`
- preparar a base para que a shell altere esse estado nos proximos passos

Neste momento, o foco ainda nao e adicionar os comandos `led on`, `led off` e `led period <ms>`.

O foco agora e fazer a task do LED passar a depender de um controle compartilhado.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 4 define como terceiro passo:

1. spawnar `led_task` com `LedControl`

Isso vem logo depois de:

1. extrair o loop do LED para `app/led_task.rs`
2. criar `LedControl`

Entao este passo e exatamente a ligacao entre essas duas partes que ja existem.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/led_task.rs`
- `src/main.rs`

Opcionalmente, se quiser manter a organizacao explicita no topo do crate, voce tambem pode ajustar pequenos imports em:

- `src/app/mod.rs`

Mas a mudanca real fica em `led_task.rs` e `main.rs`.

---

## Visao geral da ideia

No fim do passo 2, o projeto ja tem duas partes separadas:

- uma task do LED em `src/app/led_task.rs`
- uma struct `LedControl` no mesmo arquivo

Mas essas duas partes ainda nao conversam entre si.

Hoje a task do LED continua com um comportamento fixo:

- sempre habilitada
- sempre com `500 ms` ligado
- sempre com `500 ms` desligado

O objetivo do passo 3 e fazer a task ler o estado de `LedControl`, mesmo que esse estado ainda nao seja alterado pela shell.

---

## Escolha simples para este passo

Para este passo, a opcao mais segura e usar um estado global compartilhado, no mesmo estilo do `MONITOR`.

Em vez de tentar passar ownership complicado para a task, a solucao mais direta e:

1. criar um `static` global para o LED
2. proteger esse estado com `Mutex`
3. fazer a task ler esse estado em cada ciclo

Essa escolha combina com a estrutura que o projeto ja usa hoje.

---

## 1. Tornar `LedControl` visivel fora de `led_task.rs`

### Onde aplicar

Faca isso em `src/app/led_task.rs`.

### O que verificar

Como `main.rs` vai precisar declarar uma instancia global usando `LedControl`, a struct precisa ser visivel fora do modulo.

Ela deve continuar como:

```rust
pub struct LedControl {
    enabled: bool,
    period_ms: u32,
}
```

### O que isso resolve

- permite que `main.rs` use `LedControl` no tipo de uma variavel global
- mantem a struct no modulo mais natural do LED

---

## 2. Declarar um controle global do LED em `src/main.rs`

### Onde aplicar

Faca isso em `src/main.rs`, perto da declaracao do `MONITOR`.

### Como localizar o ponto exato

Hoje o arquivo ja possui um bloco como este:

```rust
pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());
```

O ponto mais natural e declarar o controle do LED logo abaixo desse bloco.

### Estrutura recomendada

Use a mesma estrategia do monitor:

```rust
pub(crate) static LED_CONTROL: Mutex<CriticalSectionRawMutex, LedControl> =
    Mutex::new(LedControl::new());
```

### Imports necessarios

Para isso, `src/main.rs` precisa importar `LedControl`:

```rust
use crate::app::led_task::{led_task, LedControl};
```

Se hoje estiver separado em duas linhas, o importante e que `LedControl` tambem seja importado.

### Observacao importante

Neste passo, voce ainda nao precisa expor metodos de shell para alterar `LED_CONTROL`.

O objetivo aqui e so disponibilizar esse estado para a task do LED.

---

## 3. Fazer a task do LED ler `LedControl`

### Onde aplicar

Faca isso em `src/app/led_task.rs`.

### Ideia principal

Hoje a task faz um blink fixo.

Agora ela deve, em cada ciclo:

1. abrir o controle compartilhado do LED
2. copiar os valores necessarios
3. liberar o lock cedo
4. agir com base nesses valores

### Por que copiar e liberar cedo

Assim como foi feito no comando `tasks`, isso evita segurar o `Mutex` por mais tempo do que o necessario.

Ou seja, a task nao deve manter o lock durante o `Timer::after_millis(...)`.

### Estrutura sugerida

Conceitualmente, a task deve evoluir para algo assim:

```rust
#[embassy_executor::task]
pub async fn led_task(pa5: Peri<'static, peripherals::PA5>) {
    let mut led = Output::new(pa5, Level::High, Speed::Low);

    loop {
        let (enabled, period_ms) = {
            let control = crate::LED_CONTROL.lock().await;
            (control.is_enabled(), control.period_ms())
        };

        if enabled {
            led.set_high();
            mark_led_execution().await;
            Timer::after_millis((period_ms / 2) as u64).await;

            led.set_low();
            Timer::after_millis((period_ms / 2) as u64).await;
        } else {
            led.set_low();
            Timer::after_millis(100).await;
        }
    }
}
```

### O que muda na pratica

- se `enabled == true`, a task faz o blink normal
- se `enabled == false`, a task mantem o LED desligado
- o periodo deixa de ser fixo e passa a vir de `LedControl`

---

## 4. Preservar o comportamento atual como estado inicial

### O que nao pode mudar ainda

Mesmo que a task passe a ler `LedControl`, o comportamento observado no hardware ainda deve continuar igual logo apos o boot.

Isso ja fica garantido se `LedControl::new()` continuar retornando:

```rust
enabled: true,
period_ms: 1000,
```

### Por que isso importa

O passo 3 deve ser uma integracao estrutural.

Ele nao deve mudar o comportamento final do LED antes de a shell começar a controlar esse estado.

---

## 5. Manter a marcacao de metricas do LED

### O que nao pode se perder

Desde a Fase 3, o comando `tasks` depende da marcacao da task do LED.

Entao, mesmo com `LedControl`, a task ainda precisa chamar:

```rust
mark_led_execution().await;
```

### Em que ponto chamar

Para manter coerencia com a implementacao atual, a melhor escolha e continuar marcando no inicio da metade ativa do blink.

Exemplo:

```rust
if enabled {
    led.set_high();
    mark_led_execution().await;
    Timer::after_millis((period_ms / 2) as u64).await;
    // ...
}
```

### Observacao importante

Se `enabled == false`, em geral nao faz sentido contar isso como um ciclo normal de blink.

Entao, neste passo, a abordagem mais limpa e nao chamar `mark_led_execution()` enquanto o LED estiver desabilitado.

---

## 6. Atualizar o spawn em `src/main.rs`

### O que verificar

No estado atual do projeto, `main.rs` ja faz:

```rust
spawner.spawn(unwrap!(led_task(p.PA5)));
```

### O que muda neste passo

Se voce seguir a estrategia de `LED_CONTROL` global, o spawn pode continuar exatamente assim.

Ou seja, o nome do passo diz "spawnar `led_task` com `LedControl`", mas a forma mais simples aqui nao e passar `LedControl` como parametro direto.

Em vez disso, a task e spawnada normalmente e passa a consultar o estado global compartilhado.

### Observacao importante

Isso continua atendendo o objetivo do passo, porque a task passa a trabalhar com `LedControl`, mesmo sem receber ownership dele como argumento.

---

## 7. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- comando `led` na shell
- parse de `led on`
- parse de `led off`
- parse de `led period <ms>`
- botao alterando `LED_CONTROL`

Tudo isso entra no passo 4 da Fase 4.

Neste passo 3, o sucesso e apenas:

- `LedControl` passa a ser usado pela task
- existe um estado compartilhado do LED
- o LED continua piscando no boot
- a base para a shell fica pronta

---

## 8. Checklist de validacao

Antes de seguir para o passo 4, valide pelo menos isto:

1. `cargo check`
2. `cargo run`
3. o LED continua piscando normalmente apos o boot
4. o comando `tasks` continua mostrando metricas de `led`
5. a shell continua funcionando como antes

### Resultado esperado

Se este passo foi feito corretamente:

- o LED continua funcional
- o comportamento inicial permanece igual
- a task agora le `LedControl`
- o proximo passo pode focar apenas nos comandos `led ...`

---

## Resumo deste passo

O passo 3 da Fase 4 nao e sobre shell ainda.

Ele e sobre ligar a task do LED ao estado `LedControl` que foi criado no passo 2.

A forma mais simples e coerente com o projeto atual e:

- declarar `LED_CONTROL` como `static` global em `main.rs`
- proteger esse estado com `Mutex`
- fazer `led_task` ler `enabled` e `period_ms` a cada ciclo
- manter o lock aberto apenas pelo tempo necessario para copiar os valores

Com isso, o firmware fica pronto para receber `led on`, `led off` e `led period <ms>` no passo seguinte.
