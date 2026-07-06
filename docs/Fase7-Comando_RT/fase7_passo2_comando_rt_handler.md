## Fase 7 - Passo 2: criar o comando `rt` e configurar intervalos esperados

Este documento cobre o passo 2 da Fase 7.

Objetivo do passo:

- configurar `expected_interval_ms` para as tasks periodicas (`led` e `adc`) em `main.rs`
- criar o comando `rt` no shell
- exibir informacoes de tempo real: nome, ultimo intervalo, intervalo esperado, jitter medio, execucoes

---

## 1. O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 7 define para o comando `rt`:

- periodicidade esperada do blink
- ultima ativacao
- jitter aproximado
- contagem de execucoes por janela

E tambem:

> LED blink usado como referencia de estabilidade

---

## 2. O que ja esta pronto do passo 1

O `TaskMetrics` ja tem:

- `expected_interval_ms` — intervalo nominal esperado
- `last_interval_ms` — ultimo intervalo observado
- `jitter_accum` / `jitter_samples` — acumulo de jitter
- `set_expected_interval()` — metodo para configurar o intervalo esperado
- `average_jitter_ms()` — metodo que retorna a media do jitter

Mas esses campos ainda estao vazios porque ninguem chamou `set_expected_interval()` ainda.

---

## 3. O que este passo deve entregar

1. Em `main.rs`, configurar o intervalo esperado do LED e do ADC logo apos a criacao do `MONITOR`
2. Em `shell.rs`, adicionar `"rt"` na tabela `COMMANDS`
3. Em `shell.rs`, implementar o handler do comando `rt`
4. O comando `rt` deve copiar as metricas, liberar o lock, e formatar a saida

---

## 4. Onde configurar os intervalos esperados em `main.rs`

No `main.rs`, o `MONITOR` e criado como static. Nao e possivel chama-lo diretamente antes do boot.

A melhor estrategia e configurar os intervalos esperados no inicio da `main`, logo apos a inicializacao do hardware, antes do `spawn` das tasks.

Exemplo:

```rust
// No inicio de main(), apos init do hardware e antes dos spawns:
{
    let mut monitor = MONITOR.lock().await;
    // LED: periodo padrao de 1000 ms configurado em LedControl.
    monitor.led.set_expected_interval(1000);
    // ADC: lendo a cada 500 ms.
    monitor.adc.set_expected_interval(500);
    // Button: nao tem periodicidade fixa, fica como None.
}
```

### Por que o LED espera 1000 ms e nao 500 ms

O `led_task` em `led_task.rs` faz um ciclo completo de `HIGH + LOW`.

O periodo total do blink e o valor de `period_ms` no `LedControl`.

O valor padrao e `1000 ms`, ou seja, `500 ms` HIGH + `500 ms` LOW.

Cada marcacao de `mark_led_execution()` acontece uma vez por ciclo completo, entao o intervalo esperado entre marcacoes e `1000 ms`.

### Por que o ADC espera 500 ms

A `adc_task` em `main.rs` faz:

```rust
loop {
    let _measured = adc.blocking_read(...);
    mark_adc_execution().await;
    Timer::after_millis(500).await;
}
```

O intervalo entre marcacoes e `500 ms`.

### E o botao?

O `button_task` nao e periodica. Ela so executa quando o usuario pressiona o botao.

Nao faz sentido configurar `expected_interval_ms` para ela.

---

## 5. Local recomendado no codigo para a configuracao

No `main.rs`, procure o bloco que configura o runtime, antes dos `spawner.spawn()`.

Um bom local e logo depois da inicializacao dos perifericos e antes de spawnea-los:

```rust
// === 5. Runtime concorrente ===

// Configura intervalos esperados para tasks periodicas.
{
    let mut monitor = MONITOR.lock().await;
    monitor.led.set_expected_interval(1000);
    monitor.adc.set_expected_interval(500);
}

// Cada periférico
spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
spawner.spawn(unwrap!(button_task(button)));
```

---

## 6. Adicionar o comando `rt` na tabela `COMMANDS`

No `shell.rs`, localize a tabela `COMMANDS` e adicione uma entrada no final:

```rust
pub const COMMANDS: &[CommandEntry] = &[
    // ... entradas existentes ...
    CommandEntry {
        name: "rt",
        help: "Exibe metricas de tempo real: intervalo, jitter, execucoes",
    },
];
```

---

## 7. Implementar o handler do `rt` no `execute_command()`

No `shell.rs`, o handler do `rt` segue o mesmo padrao do `tasks`:

1. Copia as metricas sob o lock (rapido)
2. Libera o lock
3. Formata a resposta fora do lock
4. Escreve na `ShellResponse`

### Estrutura recomendada da resposta

Para cada task monitorada (adc, button, led):

```
rt:
- adc: last=502ms, expected=500ms, jitter_avg=3ms, count=142
- led: last=1003ms, expected=1000ms, jitter_avg=4ms, count=71
- button: last=N/A, expected=N/A, jitter=N/A, count=5
```

O `button` nao tem periodicidade, entao mostra `N/A` nos campos temporais.

### Codigo sugerido para o handler

```rust
"rt" => {
    if !cmd.args.is_empty() {
        let _ = out.push_str("Erro: este comando nao aceita argumentos.\r\n");
    } else {
        // Copia as metricas e libera o lock antes de formatar.
        let (adc, button, led) = {
            let monitor = MONITOR.lock().await;
            (monitor.adc, monitor.button, monitor.led)
        };

        let _ = out.push_str("Tempo real:\r\n");

        // Funcao auxiliar local para formatar uma linha de task.
        fn format_rt_line(
            out: &mut ShellResponse,
            metrics: monitor::TaskMetrics,
        ) {
            let _ = out.push_str("- ");
            let _ = out.push_str(metrics.name);
            let _ = out.push_str(": last=");
            match metrics.last_interval_ms {
                Some(ms) => let _ = write!(out, "{}", ms),
                None => let _ = out.push_str("N/A"),
            }
            let _ = out.push_str("ms, expected=");
            match metrics.expected_interval_ms {
                Some(ms) => let _ = write!(out, "{}", ms),
                None => let _ = out.push_str("N/A"),
            }
            let _ = out.push_str("ms, jitter_avg=");
            match metrics.average_jitter_ms() {
                Some(ms) => let _ = write!(out, "{}", ms),
                None => let _ = out.push_str("N/A"),
            }
            let _ = out.push_str("ms, count=");
            let _ = write!(out, "{}", metrics.execution_count);
            let _ = out.push_str("\r\n");
        }

        format_rt_line(&mut out, adc);
        format_rt_line(&mut out, button);
        format_rt_line(&mut out, led);
    }
}
```

---

## 8. O que NAO muda neste passo

- `TaskMetrics`, `SystemMonitor`, `Am2302Snapshot`, `HcSr04Snapshot` continuam exatamente como estao
- Os handlers de `tasks`, `status`, `help`, `uptime`, `mem`, `led`, `sensors`, `clean` continuam exatamente como estao
- As tasks `adc_task`, `button_task`, `led_task` nao precisam de nenhuma alteracao
- O `LedControl` nao precisa de alteracao

---

## 9. Criterio de conclusao do passo 2

Considere o passo 2 concluido quando:

- `main.rs` chamar `set_expected_interval()` para `led` (1000 ms) e `adc` (500 ms)
- `rt` estiver registrado na tabela `COMMANDS`
- O handler do `rt` existir em `execute_command()`
- O handler copiar as metricas, liberar o lock, e formatar nome, ultimo intervalo, intervalo esperado, jitter medio e contagem
- `cargo check` passar sem erros

Ainda nao e necessario neste passo:

- testar no hardware (mas e recomendado se a placa estiver disponivel)
- configurar `expected_interval_ms` para o botao (nao se aplica)
- adicionar metricas de janela temporal (ex: "ultimos 10 segundos")

---

## 10. Proximos passos

Apos este passo, o comando `rt` ja deve exibir informacoes de tempo real.

Se houver um passo 3, ele pode incluir:

- validacao em bancada com leitura real do jitter
- extensao para mostrar jitter maximo e minimo (se houver demanda)
- integracao com o comando `tasks` para mostrar jitter junto com as metricas existentes
