## Fase 7 - Passo 1: estender `TaskMetrics` para suportar jitter e intervalo esperado

Este documento cobre o passo 1 da Fase 7.

Objetivo do passo:

- adicionar os campos de tempo real na estrutura `TaskMetrics`
- preparar o calculo de jitter dentro de `mark_execution()`
- manter a estrutura generica o suficiente para atender qualquer task periodica (LED, ADC, etc.)
- nao quebrar os comandos existentes (`tasks`, `status`, `sensors`)

Neste passo, ainda nao e necessario:

- criar o comando `rt` no shell
- registrar handlers no dispatch
- alterar `main.rs`
- testar no hardware

O foco agora e apenas deixar `TaskMetrics` pronta para coletar os dados que o `rt` vai consumir depois.

---

## 1. O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 7 define como primeiro passo:

1. Em `app/monitor.rs`, estender `TaskMetrics`:
   - `expected_interval_ms`
   - `jitter_estimado`

2. Calcular no `mark_execution()`:
   - diferenca entre `Instant::now()` e ultimo timestamp
   - comparar com intervalo esperado (se configurado)
   - acumular jitter medio

---

## 2. Estrutura atual do `TaskMetrics`

No firmware atual, `TaskMetrics` tem estes campos:

```rust
#[derive(Debug, Clone, Copy)]
pub struct TaskMetrics
{
    pub name: &'static str,
    pub execution_count: u32,
    pub last_run_ms: Option<u64>,
    pub min_interval_ms: Option<u64>,
    pub max_interval_ms: Option<u64>,
}
```

E o metodo `mark_execution()` ja faz isto:

```rust
pub fn mark_execution(&mut self, now_ms: u64)
{
    if let Some(previous_run_ms) = self.last_run_ms {
        let interval_ms = now_ms.saturating_sub(previous_run_ms);

        self.min_interval_ms = match self.min_interval_ms {
            Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
            None => Some(interval_ms),
        };

        self.max_interval_ms = match self.max_interval_ms {
            Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
            None => Some(interval_ms),
        };
    }

    self.execution_count = self.execution_count.saturating_add(1);
    self.last_run_ms = Some(now_ms);
}
```

O que **falta** para o comando `rt`:

- um campo que guarde qual e o intervalo esperado da task
- um campo que guarde o intervalo mais recente
- um campo que acumule o desvio (jitter) para calcular a media
- logica dentro de `mark_execution()` para alimentar esses campos

---

## 3. Campos novos recomendados

Para a Fase 7, a recomendacao e adicionar estes 4 campos:

```rust
pub struct TaskMetrics
{
    // Campos existentes (mantidos)
    pub name: &'static str,
    pub execution_count: u32,
    pub last_run_ms: Option<u64>,
    pub min_interval_ms: Option<u64>,
    pub max_interval_ms: Option<u64>,

    // --- Campos novos para a Fase 7 ---

    // Intervalo nominal esperado entre execucoes, em milissegundos.
    // Ex: 1000 para o LED com periodo de 1 s.
    // Ex: 500 para o ADC lendo a cada 500 ms.
    // Fica como None para tasks assincronas como o botao.
    pub expected_interval_ms: Option<u64>,

    // Ultimo intervalo observado entre duas execucoes consecutivas.
    // Preenchido a cada chamada de mark_execution().
    pub last_interval_ms: Option<u64>,

    // Soma acumulada dos valores absolutos de jitter.
    // jitter = |intervalo_observado - intervalo_esperado|.
    // Usado junto com jitter_samples para calcular a media.
    pub jitter_accum: u64,

    // Quantas amostras de jitter foram acumuladas ate agora.
    pub jitter_samples: u32,
}
```

### Por que `expected_interval_ms` e `Option<u64>` e nao `u64`

Nem toda task e estritamente periodica.

O botao, por exemplo, nao tem intervalo esperado porque ele so executa quando o usuario pressiona.

Para essas tasks, o campo fica como `None`, e o calculo de jitter simplesmente nao e aplicado.

### Por que `jitter_accum` e `jitter_samples` separados

Guardar a soma e a contagem separadamente permite:

- calcular a media sob demanda: `jitter_accum / jitter_samples`
- resetar o acumulo sem perder o intervalo esperado
- adicionar novas amostras sem precisar de divisao no ato da medicao

Isso e importante porque `mark_execution()` roda dentro de um lock do `MONITOR`. Quanto mais leve for o metodo, melhor.

---

## 4. Metodo `set_expected_interval()`

Para configurar o intervalo esperado, a recomendacao e um metodo simples:

```rust
impl TaskMetrics
{
    /// Define o intervalo nominal esperado para esta task.
    /// Ex: led_task chama com 1000 para 1 segundo de periodo.
    pub fn set_expected_interval(&mut self, interval_ms: u64)
    {
        self.expected_interval_ms = Some(interval_ms);
    }
}
```

Isso sera chamado uma unica vez durante a inicializacao do `SystemMonitor` em `main.rs`, ou diretamente apos a criacao da estrutura.

---

## 5. Logica nova dentro de `mark_execution()`

Dentro de `mark_execution()`, a mudanca principal e:

1. Guardar o intervalo calculado em `last_interval_ms`
2. Se `expected_interval_ms` estiver configurado, calcular o jitter absoluto e acumular

O esqueleto do metodo atualizado fica assim:

```rust
pub fn mark_execution(&mut self, now_ms: u64)
{
    if let Some(previous_run_ms) = self.last_run_ms {
        let interval_ms = now_ms.saturating_sub(previous_run_ms);

        // --- Atualizacao dos campos existentes ---
        self.last_interval_ms = Some(interval_ms);  // NOVO

        self.min_interval_ms = match self.min_interval_ms {
            Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
            None => Some(interval_ms),
        };

        self.max_interval_ms = match self.max_interval_ms {
            Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
            None => Some(interval_ms),
        };

        // --- Calculo de jitter (NOVO) ---
        if let Some(expected_ms) = self.expected_interval_ms {
            // jitter = desvio absoluto entre o intervalo real e o esperado.
            let jitter = if interval_ms > expected_ms {
                interval_ms - expected_ms
            } else {
                expected_ms - interval_ms
            };

            // Acumula o desvio e incrementa o contador de amostras.
            self.jitter_accum = self.jitter_accum.saturating_add(jitter);
            self.jitter_samples = self.jitter_samples.saturating_add(1);
        }
    }

    self.execution_count = self.execution_count.saturating_add(1);
    self.last_run_ms = Some(now_ms);
}
```

### Observacao sobre o calculo do jitter

A definicao usada neste passo e intencionalmente simples:

```
jitter = |intervalo_observado - intervalo_esperado|
```

Isso mede o quanto a execucao real se desviou do comportamento nominal esperado.

Para o LED, por exemplo:

- esperado: `1000 ms` (1 periodo completo do blink)
- se a task executar apos `1003 ms`, o jitter daquela amostra e `3 ms`

Isso e suficiente para o comando `rt` da Fase 7.

---

## 6. Metodo para obter o jitter medio

Para o comando `rt` consumir o jitter de forma legivel, e util ter um metodo que devolva a media:

```rust
impl TaskMetrics
{
    /// Retorna o jitter medio observado em milissegundos.
    /// Retorna None se ainda nao houver amostras ou se a task nao tiver
    /// intervalo esperado configurado.
    pub fn average_jitter_ms(&self) -> Option<u64>
    {
        if self.jitter_samples > 0 {
            Some(self.jitter_accum / self.jitter_samples as u64)
        } else {
            None
        }
    }
}
```

Esse metodo nao altera o estado interno. Ele apenas consulta os campos acumulados.

---

## 7. Atualizacao do `const fn new()`

O construtor existente precisa inicializar os novos campos com valores neutros:

```rust
pub const fn new(name: &'static str) -> Self
{
    Self
    {
        name,
        execution_count: 0,
        last_run_ms: None,
        min_interval_ms: None,
        max_interval_ms: None,

        // --- Campos novos ---
        expected_interval_ms: None,
        last_interval_ms: None,
        jitter_accum: 0,
        jitter_samples: 0,
    }
}
```

Isso garante que nenhuma task existente precise ser modificada para continuar funcionando. O comportamento dos comandos `tasks`, `status` e `sensors` permanece identico.

---

## 8. Impacto sobre o `SystemMonitor`

O `SystemMonitor` nao precisa de mudancas estruturais.

Os campos novos estao dentro de `TaskMetrics`, e o `SystemMonitor` apenas declara instancias de `TaskMetrics`:

```rust
pub struct SystemMonitor
{
    pub adc: TaskMetrics,
    pub button: TaskMetrics,
    pub led: TaskMetrics,
    pub am2302: Am2302Snapshot,
    pub hcsr04: HcSr04Snapshot,
}
```

Nao e necessario alterar nada aqui neste passo.

As configuracao de `expected_interval_ms` para cada task virá depois, em `main.rs`.

---

## 9. Forma final esperada de `TaskMetrics` apos o passo 1

```rust
#[derive(Debug, Clone, Copy)]
pub struct TaskMetrics
{
    pub name: &'static str,
    pub execution_count: u32,
    pub last_run_ms: Option<u64>,
    pub min_interval_ms: Option<u64>,
    pub max_interval_ms: Option<u64>,

    // Fase 7: campos de tempo real
    pub expected_interval_ms: Option<u64>,
    pub last_interval_ms: Option<u64>,
    pub jitter_accum: u64,
    pub jitter_samples: u32,
}

impl TaskMetrics
{
    pub const fn new(name: &'static str) -> Self
    {
        Self
        {
            name,
            execution_count: 0,
            last_run_ms: None,
            min_interval_ms: None,
            max_interval_ms: None,

            expected_interval_ms: None,
            last_interval_ms: None,
            jitter_accum: 0,
            jitter_samples: 0,
        }
    }

    pub fn set_expected_interval(&mut self, interval_ms: u64)
    {
        self.expected_interval_ms = Some(interval_ms);
    }

    pub fn mark_execution(&mut self, now_ms: u64)
    {
        if let Some(previous_run_ms) = self.last_run_ms
        {
            let interval_ms = now_ms.saturating_sub(previous_run_ms);

            self.last_interval_ms = Some(interval_ms);

            self.min_interval_ms = match self.min_interval_ms
            {
                Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
                None => Some(interval_ms),
            };

            self.max_interval_ms = match self.max_interval_ms
            {
                Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
                None => Some(interval_ms),
            };

            if let Some(expected_ms) = self.expected_interval_ms
            {
                let jitter = if interval_ms > expected_ms
                {
                    interval_ms - expected_ms
                } else {
                    expected_ms - interval_ms
                };

                self.jitter_accum = self.jitter_accum.saturating_add(jitter);
                self.jitter_samples = self.jitter_samples.saturating_add(1);
            }
        }

        self.execution_count = self.execution_count.saturating_add(1);
        self.last_run_ms = Some(now_ms);
    }

    pub fn average_jitter_ms(&self) -> Option<u64>
    {
        if self.jitter_samples > 0
        {
            Some(self.jitter_accum / self.jitter_samples as u64)
        }
        else
        {
            None
        }
    }
}
```

---

## 10. O que NAO muda neste passo

- `SystemMonitor` continua exatamente igual
- `Am2302Snapshot` e `HcSr04Snapshot` continuam exatamente iguais
- o comando `tasks` existente continua funcionando sem alteracao
- `main.rs` nao precisa ser alterado neste passo
- a funcao `mark_adc_execution()`, `mark_button_execution()` e `mark_led_execution()` continuam exatamente iguais

Nenhuma chamada existente quebra, porque o construtor inicializa os campos novos com valores neutros.

---

## 11. Criterio de conclusao do passo 1

Considere o passo 1 concluido quando:

- `expected_interval_ms` estiver declarado em `TaskMetrics`
- `last_interval_ms` estiver declarado em `TaskMetrics`
- `jitter_accum` e `jitter_samples` estiverem declarados em `TaskMetrics`
- o construtor `new()` inicializar todos os campos novos com valores neutros
- `mark_execution()` preencher `last_interval_ms` e acumular jitter quando `expected_interval_ms` estiver configurado
- `average_jitter_ms()` existir e retornar `Option<u64>`
- `cargo check` passar sem erros

Ainda nao e necessario neste passo:

- criar o comando `rt` no shell
- configurar `expected_interval_ms` para nenhuma task
- testar no hardware
- alterar `main.rs`

---

## 12. Proximo passo

O passo 2 da Fase 7 sera:

- configurar `expected_interval_ms` para as tasks periodicas (`led` e `adc`) em `main.rs`
- criar o comando `rt` no shell
- adicionar a entrada na tabela `COMMANDS`
- exibir:
  - nome da task
  - ultimo intervalo observado
  - intervalo esperado
  - jitter medio
  - contagem de execucoes
