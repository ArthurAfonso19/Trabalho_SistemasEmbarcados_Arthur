## Fase 3 - Passo 2: Estrutura `SystemMonitor`

Este arquivo descreve o segundo passo da Fase 3.

Objetivo deste passo:

- criar um container central para guardar as metricas das tasks
- organizar varias instancias de `TaskMetrics` em um unico lugar
- preparar a integracao futura com `main.rs` e com os comandos da shell

Neste passo, o foco ainda nao e expor nada no console.
O foco e criar a estrutura `SystemMonitor` dentro de `src/app/monitor.rs`.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 3 define como segundo passo:

1. criar `SystemMonitor` como container de metricas

Entao, depois de criar `TaskMetrics` no passo 1, agora precisamos de uma estrutura que agrupe as metricas do sistema.

---

## Arquivo alterado neste passo

Neste passo, a implementacao fica concentrada em:

- `src/app/monitor.rs`

Neste momento, nao e necessario alterar:

- `src/app/mod.rs`
- `src/main.rs`
- `src/app/shell.rs`

---

## Visao geral da ideia

No passo 1, cada task ganhou o formato de uma metrica individual:

- `TaskMetrics`

Agora, no passo 2, vamos juntar essas metricas em uma estrutura maior:

- `SystemMonitor`

Essa estrutura vai funcionar como um ponto central para armazenar as metricas de tasks como:

- `adc`
- `button`
- `led`

Mais adiante, a shell podera consultar esse container para montar a resposta do comando `tasks`.

---

## Escolha simples para este passo

Para manter a implementacao pequena e previsivel, a versao inicial de `SystemMonitor` vai ter campos fixos.

Exemplo:

- `adc`
- `button`
- `led`

Essa escolha e boa nesta etapa porque:

- evita estruturas dinamicas desnecessarias
- combina bem com firmware `no_std`
- deixa o codigo mais facil de entender
- simplifica a futura integracao com a shell

---

## 1. Alterar `src/app/monitor.rs`

### Onde aplicar

Faca isso em `src/app/monitor.rs`.

### Como localizar no arquivo

Procure o final do bloco `impl TaskMetrics`.

Em outras palavras:

- encontre o fechamento desta parte:

```rust
impl TaskMetrics {
    // ...
}
```

- depois desse bloco, adicione a nova struct `SystemMonitor`

---

## 2. Adicionar `SystemMonitor`

### Antes

Hoje o arquivo termina apenas com `TaskMetrics` e seu `impl`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct TaskMetrics {
    pub name: &'static str,
    pub execution_count: u32,
    pub last_run_ms: Option<u64>,
    pub min_interval_ms: Option<u64>,
    pub max_interval_ms: Option<u64>,
}

impl TaskMetrics {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            execution_count: 0,
            last_run_ms: None,
            min_interval_ms: None,
            max_interval_ms: None,
        }
    }

    pub fn mark_execution(&mut self, now_ms: u64) {
        // ...
    }
}
```

### Depois

Faca o arquivo ficar assim, adicionando `SystemMonitor` apos o `impl TaskMetrics`:

```rust
#[derive(Debug, Clone, Copy)]
// Debug permite inspecionar a struct em logs e depuracao.
// Clone + Copy permitem copiar a struct por valor sem complexidade extra.
pub struct TaskMetrics {
    pub name: &'static str,
    // Nome fixo da task, por exemplo: "adc" ou "button".

    pub execution_count: u32,
    // Quantas vezes a task marcou execucao.

    pub last_run_ms: Option<u64>,
    // Timestamp da ultima execucao em milissegundos.

    pub min_interval_ms: Option<u64>,
    // Menor intervalo observado entre duas execucoes consecutivas.

    pub max_interval_ms: Option<u64>,
    // Maior intervalo observado entre duas execucoes consecutivas.
}

impl TaskMetrics {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            execution_count: 0,
            last_run_ms: None,
            min_interval_ms: None,
            max_interval_ms: None,
        }
    }

    pub fn mark_execution(&mut self, now_ms: u64) {
        // Se ja havia uma execucao anterior, calcula o intervalo desde ela.
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
}

pub struct SystemMonitor {
    pub adc: TaskMetrics,
    // Metricas da task de ADC.

    pub button: TaskMetrics,
    // Metricas da task do botao.

    pub led: TaskMetrics,
    // Metricas da task de LED.
}

impl SystemMonitor {
    pub const fn new() -> Self {
        Self {
            adc: TaskMetrics::new("adc"),
            // Inicializa a entrada do ADC com nome fixo.

            button: TaskMetrics::new("button"),
            // Inicializa a entrada do botao com nome fixo.

            led: TaskMetrics::new("led"),
            // Inicializa a entrada do LED com nome fixo.
        }
    }
}
```

---

## 3. O que exatamente adicionar

### Como aplicar no arquivo

Faca o exemplo a seguir apos o fechamento de `impl TaskMetrics`.

```rust
pub struct SystemMonitor {
    pub adc: TaskMetrics,
    // Metricas da task de ADC.

    pub button: TaskMetrics,
    // Metricas da task do botao.

    pub led: TaskMetrics,
    // Metricas da task de LED.
}

impl SystemMonitor {
    pub const fn new() -> Self {
        Self {
            adc: TaskMetrics::new("adc"),
            // Inicializa a entrada do ADC com nome fixo.

            button: TaskMetrics::new("button"),
            // Inicializa a entrada do botao com nome fixo.

            led: TaskMetrics::new("led"),
            // Inicializa a entrada do LED com nome fixo.
        }
    }
}
```

---

## 4. Explicando cada parte de `SystemMonitor`

### `pub struct SystemMonitor`

```rust
pub struct SystemMonitor {
    pub adc: TaskMetrics,
    pub button: TaskMetrics,
    pub led: TaskMetrics,
}
```

Essa struct funciona como um agrupador.

Em vez de ter metricas soltas no codigo, passamos a ter um objeto unico que concentra o estado monitorado do sistema.

Na pratica:

- `adc` guarda as metricas da task de ADC
- `button` guarda as metricas da task do botao
- `led` guarda as metricas da task de LED

---

### Por que usar campos fixos

Neste projeto, o conjunto inicial de tasks conhecidas e pequeno e estavel.

Por isso, usar campos nomeados e uma solucao boa nesta fase.

Vantagens:

- mais simples que vetor ou mapa
- sem alocacao dinamica
- mais facil de consultar no codigo
- combina com a ideia de firmware com estrutura conhecida em tempo de compilacao

---

### `pub const fn new() -> Self`

```rust
pub const fn new() -> Self {
    Self {
        adc: TaskMetrics::new("adc"),
        button: TaskMetrics::new("button"),
        led: TaskMetrics::new("led"),
    }
}
```

Esse metodo cria um `SystemMonitor` completo, ja com cada entrada inicializada.

Ele e util porque garante que:

- todas as metricas comecem em estado valido
- os nomes das tasks ja fiquem cadastrados
- a inicializacao futura em `static` ou compartilhamento seja mais simples

---

### Explicacao de `Self`

```rust
Self {
    adc: TaskMetrics::new("adc"),
    button: TaskMetrics::new("button"),
    led: TaskMetrics::new("led"),
}
```

`Self` e uma forma curta de se referir ao proprio tipo do `impl` atual.

Nesse caso:

- dentro de `impl SystemMonitor`, `Self` significa `SystemMonitor`

Entao estes dois formatos seriam equivalentes:

```rust
Self { /* ... */ }
```

```rust
SystemMonitor { /* ... */ }
```

Usar `Self` e comum em Rust e ajuda a deixar o codigo mais compacto.

---

## 5. Exemplo de uso futuro

Mais adiante, a ideia sera algo como:

```rust
let mut monitor = SystemMonitor::new();

// Em uma etapa futura, cada task atualizara seu proprio campo.
monitor.adc.mark_execution(500);
monitor.button.mark_execution(1200);
monitor.led.mark_execution(1500);
```

Depois disso, `monitor` passa a concentrar todas as metricas principais.

Esse e o passo que prepara o comando `tasks` da shell, porque agora existe um lugar unico para consultar o estado das tasks.

---

## 6. O que ainda nao entra neste passo

Mesmo criando `SystemMonitor`, ainda nao e necessario neste passo:

- compartilhar a struct entre tasks
- usar `Mutex`
- usar `StaticCell`
- atualizar `main.rs`
- expor o comando `tasks`
- calcular `uptime`
- calcular `mem`

Tudo isso fica para os proximos passos da Fase 3.

---

## 7. Resultado esperado deste passo

Ao final deste passo, o projeto deve ter:

- `TaskMetrics` ja existente no `monitor.rs`
- `SystemMonitor` agrupando as metricas principais
- um construtor `SystemMonitor::new()`

Ainda nao havera comportamento novo visivel no console, porque este passo continua sendo de infraestrutura.

---

## 8. Resumo direto

Neste passo, voce deve:

1. abrir `src/app/monitor.rs`
2. localizar o fim de `impl TaskMetrics`
3. adicionar `SystemMonitor`
4. adicionar `SystemMonitor::new()`
5. inicializar `adc`, `button` e `led` com `TaskMetrics::new(...)`

O foco aqui nao e a shell ainda, e sim organizar as metricas em um container unico para uso futuro.
