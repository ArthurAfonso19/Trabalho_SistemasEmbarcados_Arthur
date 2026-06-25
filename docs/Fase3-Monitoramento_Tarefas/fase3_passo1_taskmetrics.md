## Fase 3 - Passo 1: Estrutura `TaskMetrics`

Este arquivo descreve o primeiro passo da Fase 3.

Objetivo deste passo:

- criar a base de monitoramento das tasks
- definir uma estrutura simples para guardar metricas
- preparar o projeto para expor essas metricas pela shell nas proximas etapas

Neste passo, o foco nao e integrar tudo no firmware ainda.
O foco e criar a estrutura `TaskMetrics` em um novo modulo `monitor.rs`.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 3 comeca assim:

1. em `app/monitor.rs`, criar `TaskMetrics`
2. essa estrutura deve ter:
   - nome
   - contador
   - timestamp da ultima execucao
   - intervalo minimo/maximo
   - `mark_execution()` para registrar

Entao este passo e a fundacao do monitoramento.

---

## Arquivos alterados neste passo

Neste passo, a implementacao fica concentrada em:

- `src/app/mod.rs`
- `src/app/monitor.rs`

Ainda nao e necessario alterar `src/main.rs` nem `src/app/shell.rs`.

---

## Visao geral da ideia

Cada task monitorada vai ter uma estrutura com informacoes como:

- nome da task
- quantas vezes ela ja executou
- quando foi a ultima execucao
- menor intervalo observado entre execucoes
- maior intervalo observado entre execucoes

Depois, cada task podera chamar algo como:

```rust
metrics.mark_execution(agora_ms);
```

Com isso, a propria task registra sua atividade, e a shell podera consultar os dados depois.

---

## Escolha simples para este passo

Para manter o passo pequeno e seguro, a estrutura abaixo usa `u64` para tempo em milissegundos.

Isso significa:

- `last_run_ms` guarda o instante da ultima execucao
- `min_interval_ms` guarda o menor intervalo entre duas execucoes
- `max_interval_ms` guarda o maior intervalo entre duas execucoes

Nesta etapa, isso e suficiente e evita acoplamento desnecessario com outros tipos mais complexos.

---

## 1. Criar o novo modulo `monitor`

### Onde aplicar

Faca isso em `src/app/mod.rs`.

### Como localizar no arquivo

Procure o arquivo `src/app/mod.rs`.

Se hoje ele estiver assim, voce precisa adicionar a exportacao do novo modulo.

### Antes

```rust
pub mod shell;
```

### Depois

```rust
pub mod monitor;
pub mod shell;
```

### O que mudou

```rust
pub mod monitor;
// Declara o novo modulo de monitoramento em src/app/monitor.rs.

pub mod shell;
// Mantem o modulo atual da shell disponivel para o resto do projeto.
```

### Explicacao da diretiva `mod`

Em Rust, `mod` diz ao compilador que existe um modulo com aquele nome.

Neste caso:

- `pub mod monitor;` faz o compilador carregar `src/app/monitor.rs`
- `pub` torna esse modulo acessivel fora de `app`

Sem isso, o arquivo pode ate existir, mas o resto do projeto nao consegue usar `app::monitor`.

---

## 2. Criar `src/app/monitor.rs`

### Onde aplicar

Crie um novo arquivo chamado `src/app/monitor.rs`.

### Conteudo completo sugerido

```rust
#[derive(Debug, Clone, Copy)]
// Debug permite inspecionar a struct em logs e depuracao.
// Clone + Copy permitem copiar a struct por valor sem complexidade extra.
pub struct TaskMetrics {
    pub name: &'static str,
    // Nome fixo da task, por exemplo: "adc" ou "button".
    // &'static str aponta para uma string literal que vive o programa inteiro.

    pub execution_count: u32,
    // Quantas vezes a task marcou execucao.

    pub last_run_ms: Option<u64>,
    // Timestamp da ultima execucao em milissegundos.
    // Option e usada porque, no inicio, a task ainda pode nao ter executado.

    pub min_interval_ms: Option<u64>,
    // Menor intervalo observado entre duas execucoes consecutivas.
    // Tambem comeca como None porque ainda nao existe intervalo antes da segunda execucao.

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
            // saturating_sub evita underflow caso now_ms seja menor que previous_run_ms.

            self.min_interval_ms = match self.min_interval_ms {
                Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
                None => Some(interval_ms),
            };
            // Na primeira medicao de intervalo, inicializa o minimo.
            // Depois disso, mantem sempre o menor valor observado.

            self.max_interval_ms = match self.max_interval_ms {
                Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
                None => Some(interval_ms),
            };
            // Na primeira medicao de intervalo, inicializa o maximo.
            // Depois disso, mantem sempre o maior valor observado.
        }

        self.execution_count = self.execution_count.saturating_add(1);
        // saturating_add evita overflow em execucoes muito longas.

        self.last_run_ms = Some(now_ms);
        // Registra o timestamp da execucao mais recente.
    }
}
```

---

## 3. Explicando cada parte da struct

### `#[derive(Debug, Clone, Copy)]`

```rust
#[derive(Debug, Clone, Copy)]
```

Isso pede ao compilador para gerar implementacoes automaticas.

- `Debug`: permite imprimir a struct em contexto de depuracao
- `Clone`: permite duplicar explicitamente a struct
- `Copy`: permite copia implicita por valor, porque todos os campos sao simples

Para esta estrutura, isso e util porque ela so guarda valores pequenos e triviais.

---

### `name: &'static str`

```rust
pub name: &'static str,
```

Esse campo guarda o nome da task.

Exemplos:

```rust
"adc"
"button"
"led"
```

O tipo `&'static str` significa:

- `str`: texto UTF-8
- `&`: referencia para esse texto
- `'static`: o texto vive durante toda a execucao do programa

Isso combina bem com nomes fixos de tasks, porque normalmente eles sao literais no codigo.

---

### `Option<u64>`

```rust
pub last_run_ms: Option<u64>,
pub min_interval_ms: Option<u64>,
pub max_interval_ms: Option<u64>,
```

`Option<T>` e usado quando um valor pode ou nao existir.

No nosso caso:

- antes da primeira execucao, `last_run_ms` ainda nao existe
- antes da segunda execucao, nao existe intervalo minimo/maximo para comparar

Por isso, usamos:

- `None` quando ainda nao ha dado valido
- `Some(valor)` quando o campo ja foi preenchido

Isso e melhor do que inventar valores artificiais como `0` para “sem valor”.

---

### `pub const fn new(...)`

```rust
pub const fn new(name: &'static str) -> Self
```

Essa funcao constroi uma instancia inicial de `TaskMetrics`.

O `const fn` significa que ela pode ser usada tambem em contextos constantes, se isso vier a ser util depois.

Exemplo:

```rust
let metrics = TaskMetrics::new("adc");
```

Estado inicial:

- contador em `0`
- ultimo timestamp em `None`
- min/max intervalo em `None`

---

### `impl TaskMetrics`

```rust
impl TaskMetrics {
    // ...
}
```

`impl` e o bloco onde definimos os metodos associados a struct.

Aqui, estamos dizendo:

- como criar a struct com `new(...)`
- como atualiza-la com `mark_execution(...)`

---

## 4. Explicando `mark_execution()`

### Codigo

```rust
pub fn mark_execution(&mut self, now_ms: u64) {
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

### O que esta funcao faz

Toda vez que a task rodar, ela chama esse metodo passando o tempo atual em milissegundos.

Exemplo:

```rust
metrics.mark_execution(1500);
```

Esse metodo:

1. verifica se ja existia uma execucao anterior
2. se existia, calcula o intervalo entre a anterior e a atual
3. atualiza o menor intervalo
4. atualiza o maior intervalo
5. incrementa o contador total
6. guarda o timestamp atual como ultimo timestamp

---

### Explicacao de `&mut self`

```rust
pub fn mark_execution(&mut self, now_ms: u64)
```

`self` representa a propria instancia da struct.

`&mut self` significa:

- a funcao recebe uma referencia para a propria struct
- essa referencia e mutavel
- entao o metodo pode alterar os campos internos

Isso e necessario porque `mark_execution()` muda:

- `execution_count`
- `last_run_ms`
- `min_interval_ms`
- `max_interval_ms`

---

### Explicacao de `if let Some(...)`

```rust
if let Some(previous_run_ms) = self.last_run_ms {
    // ...
}
```

Isso significa:

- se `last_run_ms` tiver valor, entre no bloco
- se estiver `None`, nao faz nada

Na pratica:

- primeira execucao: ainda nao existe valor anterior
- segunda execucao em diante: ja da para calcular intervalo

---

### Explicacao de `match`

```rust
self.min_interval_ms = match self.min_interval_ms {
    Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
    None => Some(interval_ms),
};
```

Aqui o `match` trata os dois estados possiveis de `Option<u64>`.

- se ja havia um minimo anterior, compara com o novo intervalo
- se nao havia, usa o intervalo atual como primeiro minimo

O mesmo raciocinio vale para `max_interval_ms`.

---

### Explicacao de `saturating_sub` e `saturating_add`

```rust
let interval_ms = now_ms.saturating_sub(previous_run_ms);
self.execution_count = self.execution_count.saturating_add(1);
```

Esses metodos evitam overflow/underflow numerico.

- `saturating_sub`: se a subtracao fosse negativa, o resultado satura em `0`
- `saturating_add`: se a soma estourasse o limite do tipo, ela satura no valor maximo

Para firmware, isso e uma protecao simples e barata.

---

## 5. Exemplo de uso futuro

Mais adiante, dentro de uma task, a ideia sera algo assim:

```rust
// Exemplo conceitual para uma etapa futura.
let now_ms = 2_500;
metrics.mark_execution(now_ms);
```

Depois de varias execucoes, a struct pode ficar com valores como:

```rust
TaskMetrics {
    name: "adc",
    execution_count: 10,
    last_run_ms: Some(5000),
    min_interval_ms: Some(498),
    max_interval_ms: Some(503),
}
```

Esses dados sao exatamente o tipo de informacao que o comando `tasks` vai mostrar na shell nas proximas etapas.

---

## 6. Implementacao minima recomendada

Se voce quiser manter este passo bem enxuto, implemente somente:

1. `pub mod monitor;` em `src/app/mod.rs`
2. a struct `TaskMetrics` em `src/app/monitor.rs`
3. os metodos `new(...)` e `mark_execution(...)`

Ainda nao precisa:

- integrar com `main`
- usar `Mutex`
- compartilhar metricas entre tasks
- adicionar comandos novos na shell

Essas partes ficam para os proximos passos da Fase 3.

---

## 7. Resultado esperado deste passo

Ao final deste passo, o projeto deve ter:

- um novo modulo `app::monitor`
- uma struct `TaskMetrics` compilavel
- uma base pronta para instrumentar tasks reais

Ainda nao havera novo comportamento visivel no console, porque este passo prepara apenas a infraestrutura.

---

## 8. Resumo direto

Neste passo, voce deve:

1. declarar `pub mod monitor;` em `src/app/mod.rs`
2. criar `src/app/monitor.rs`
3. implementar `TaskMetrics`
4. adicionar `new(...)`
5. adicionar `mark_execution(...)`

O foco aqui nao e a shell em si, e sim a base de dados que a shell vai consultar depois.
