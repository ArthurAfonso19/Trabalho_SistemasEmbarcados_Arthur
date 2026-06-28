## Fase 4 - Passo 2: Criar `LedControl`

Este arquivo descreve o segundo passo da Fase 4.

Objetivo deste passo:

- criar uma estrutura para representar o estado configuravel do LED
- separar o estado do LED da logica da task
- preparar a base para controle via shell nos proximos passos

Neste momento, o foco ainda nao e implementar os comandos `led on`, `led off` e `led period <ms>`.

O foco agora e apenas criar a estrutura `LedControl` com campos e metodos simples.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 4 define como segundo passo:

1. criar `LedControl`
2. com os campos:
   - `enabled: bool`
   - `period_ms: u32`
3. com os metodos:
   - `toggle`
   - `set_period`
   - `is_enabled`

Entao este passo ainda nao pede:

- shell controlando o LED
- parser de subcomandos `led`
- compartilhamento global definitivo com `main.rs`
- refatorar o botao para controlar o LED

Essas partes pertencem aos passos seguintes da Fase 4.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/app/led_task.rs`

Dependendo de como voce organizar os imports, tambem pode haver ajuste pequeno em:

- `src/main.rs`

Mas, para este passo 2 isoladamente, a melhor escolha e manter a mudanca principal dentro de `src/app/led_task.rs`.

---

## Visao geral da ideia

No passo 1, o LED virou uma task formal separada.

Agora falta criar uma representacao do estado configuravel dessa task.

Esse estado precisa dizer, no minimo:

- se o LED deve estar habilitado ou nao
- qual periodo de blink deve ser usado

Mesmo que a shell ainda nao altere esses valores, a estrutura ja precisa existir para que o restante da Fase 4 tenha uma base limpa.

---

## Escolha simples para este passo

Para este passo, a opcao mais segura e criar `LedControl` como uma struct simples, sem `Mutex`, sem atomics e sem integracao completa ainda.

Exemplo da ideia:

```rust
pub struct LedControl {
    enabled: bool,
    period_ms: u32,
}
```

Isso e suficiente agora porque o objetivo ainda nao e resolver acesso concorrente.

O objetivo e apenas definir:

- quais dados representam o estado do LED
- quais metodos publicos vao manipular esse estado

---

## 1. Onde implementar `LedControl`

### Onde aplicar

Faca isso em `src/app/led_task.rs`.

### Por que esse arquivo e o melhor lugar agora

No estado atual do projeto:

- a task do LED ja mora em `src/app/led_task.rs`
- o controle pertence diretamente a essa tarefa
- ainda nao existe necessidade real de criar outro modulo separado

Entao, para manter a mudanca pequena e coerente com o projeto, o melhor e deixar `LedControl` no mesmo arquivo da task por enquanto.

### Onde escrever exatamente

Em `src/app/led_task.rs`, escreva a struct acima da funcao `led_task(...)`.

Ou seja, a organizacao mais natural neste passo e:

```rust
// imports...

pub struct LedControl {
    // ...
}

impl LedControl {
    // ...
}

#[embassy_executor::task]
pub async fn led_task(...) {
    // ...
}
```

---

## 2. Criar a struct `LedControl`

### Campos pedidos pelo plano

O plano pede exatamente estes dois campos:

```rust
pub struct LedControl {
    enabled: bool,
    period_ms: u32,
}
```

### O que cada campo significa

- `enabled`
  - `true`: a task pode manter o comportamento normal do LED
  - `false`: nos proximos passos a task podera manter o LED desligado

- `period_ms`
  - representa o periodo total do blink em milissegundos
  - por exemplo, `1000` significa um ciclo completo de 1 segundo

### Observacao importante

Neste passo, o campo `period_ms` deve nascer coerente com o comportamento que ja existe no firmware.

Hoje o LED faz:

- `500 ms` ligado
- `500 ms` desligado

Entao o periodo total coerente para `LedControl` e:

```rust
1000
```

---

## 3. Criar um construtor simples

### Por que vale a pena

Mesmo que o plano nao cite explicitamente `new()`, vale muito a pena criar um construtor pequeno para garantir um estado inicial consistente.

### Forma recomendada

Crie um `new()` simples assim:

```rust
impl LedControl {
    pub const fn new() -> Self {
        Self {
            enabled: true,
            period_ms: 1000,
        }
    }
}
```

### Por que esse valor inicial faz sentido

- `enabled: true`
  - preserva o comportamento atual do firmware, onde o LED ja pisca normalmente

- `period_ms: 1000`
  - preserva o periodo total atual do blink

---

## 4. Criar os metodos basicos pedidos pelo plano

### Metodos minimos

O plano cita estes metodos:

- `toggle`
- `set_period`
- `is_enabled`

Uma implementacao pequena e coerente seria:

```rust
impl LedControl {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn set_period(&mut self, period_ms: u32) {
        self.period_ms = period_ms;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
```

### Observacao importante

Mesmo o plano nao citando explicitamente, vale a pena adicionar tambem um getter para o periodo.

Nos proximos passos, a task do LED vai precisar ler `period_ms`.

Entao a forma mais pratica e acrescentar:

```rust
pub fn period_ms(&self) -> u32 {
    self.period_ms
}
```

### Estrutura completa sugerida

Uma versao pequena e suficiente para este passo seria:

```rust
pub struct LedControl {
    enabled: bool,
    period_ms: u32,
}

impl LedControl {
    pub const fn new() -> Self {
        Self {
            enabled: true,
            period_ms: 1000,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn set_period(&mut self, period_ms: u32) {
        self.period_ms = period_ms;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn period_ms(&self) -> u32 {
        self.period_ms
    }
}
```

---

## 5. O que ainda nao precisa mudar na task do LED

Neste passo 2, voce ainda nao precisa reescrever a assinatura de `led_task(...)` para receber `LedControl`.

Ou seja, ainda pode continuar algo conceitualmente assim:

```rust
#[embassy_executor::task]
pub async fn led_task(...) {
    // blink atual
}
```

### Por que nao misturar tudo agora

Porque o plano separa bem as responsabilidades:

1. passo 1: extrair a task
2. passo 2: criar `LedControl`
3. passo 3: spawnar `led_task` com `LedControl`

Se voce tentar integrar tudo agora, fica mais dificil validar onde um erro nasceu.

---

## 6. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- `static` global de `LedControl`
- `Mutex` para o LED
- `led on`
- `led off`
- `led period <ms>`
- acesso do botao ao controle do LED

Tudo isso entra depois.

Neste passo 2, o sucesso e apenas:

- `LedControl` existe
- os campos pedidos existem
- os metodos basicos existem
- o projeto continua compilando

---

## 7. Checklist de validacao

Antes de seguir para o passo 3, valide pelo menos isto:

1. `cargo check`
2. `LedControl` existe em `src/app/led_task.rs`
3. a task do LED continua compilando
4. o firmware nao perdeu o comportamento atual por causa dessa adicao estrutural

### Resultado esperado

Se este passo foi feito corretamente:

- o comportamento externo do LED ainda nao muda
- a diferenca e estrutural
- a base para o controle via shell fica pronta

---

## Resumo deste passo

O passo 2 da Fase 4 nao e sobre novos comandos nem sobre concorrencia ainda.

Ele e sobre introduzir uma estrutura `LedControl` pequena, clara e coerente com o comportamento atual do LED.

Essa struct deve nascer com a menor complexidade possivel:

- `enabled: bool`
- `period_ms: u32`
- `new()` com valores iniciais coerentes
- metodos simples para leitura e atualizacao

Com isso, o passo seguinte pode conectar a task do LED a esse estado compartilhado sem misturar tudo de uma vez.
