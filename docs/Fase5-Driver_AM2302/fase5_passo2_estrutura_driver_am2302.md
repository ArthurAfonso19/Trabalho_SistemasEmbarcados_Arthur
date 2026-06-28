## Fase 5 - Passo 2: Estrutura do driver `AM2302`

Este arquivo descreve o segundo passo da Fase 5.

Objetivo deste passo:

- criar a base do driver em `src/drivers/am2302.rs`
- definir os tipos principais antes de implementar o protocolo completo
- preparar a assinatura publica do driver para os proximos passos

Neste momento, o foco ainda nao e concluir a leitura dos `40 bits` nem integrar o sensor ao `main.rs`.

O foco agora e deixar pronta a estrutura minima do driver:

- `Am2302Error`
- `Am2302<PIN, D>`
- `new(pin, delay)`
- `read() -> Result<(f32, f32), Am2302Error>`

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 5 define como segundo passo:

1. em `src/drivers/am2302.rs`:
   - `Am2302Error` enum
   - `Am2302<PIN: OutputPin + InputPin, D: DelayUs>` struct
   - `new(pin, delay)`
   - `read() -> Result<(f32, f32), Am2302Error>`

Entao este passo ainda nao pede:

- leitura completa do protocolo
- task periodica no `main.rs`
- comando `sensors`
- integracao com a shell

Essas partes pertencem aos passos seguintes da Fase 5.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/drivers/mod.rs`
- `src/drivers/am2302.rs`
- `src/main.rs`

### Por que `src/main.rs` aparece aqui

Hoje o projeto ainda nao possui um modulo `drivers/`.

Entao, para o novo arquivo compilar e passar a existir no crate, sera necessario pelo menos declarar o modulo raiz de drivers em algum ponto da arvore do projeto.

No estado atual do projeto, a forma mais simples e:

1. criar `src/drivers/mod.rs`
2. declarar `mod drivers;` em `src/main.rs`

---

## Visao geral da ideia

No passo 1, o datasheet ja confirmou:

- como o sensor inicia a comunicacao
- como os bits sao codificados
- como o checksum e calculado
- quais cuidados eletricos devem ser respeitados

Agora o proximo movimento natural e transformar isso em uma API de driver pequena e clara.

A ideia deste passo e criar um driver com esta cara:

```rust
let mut sensor = Am2302::new(pin, delay);
let (temp_c, humidity_rh) = sensor.read().await?;
```

Mesmo que o corpo de `read()` ainda nao esteja completo neste passo, essa interface publica ja precisa estar decidida.

---

## Escolha simples para este passo

Para este projeto, a opcao mais segura e criar um driver generico com dois campos:

1. o pino bidirecional do sensor
2. um objeto de delay em microssegundos

Ou seja:

```rust
pub struct Am2302<PIN, D> {
    pin: PIN,
    delay: D,
}
```

Essa escolha e boa porque:

- nao prende o driver ao `embassy_stm32` direto
- combina com o objetivo do trabalho de manter drivers mais genericos
- deixa claro que o protocolo depende de GPIO + temporizacao fina

---

## 1. Criar a pasta e o modulo `drivers`

### Onde aplicar

Como hoje o projeto nao tem `src/drivers/`, este passo comeca criando:

```text
src/drivers/
src/drivers/mod.rs
src/drivers/am2302.rs
```

### Como organizar

No `src/drivers/mod.rs`, a forma minima e:

```rust
pub mod am2302;
```

### O que isso resolve

- cria um lugar proprio para drivers secundarios
- deixa a estrutura coerente com o plano do projeto
- permite importar `crate::drivers::am2302::Am2302`

---

## 2. Declarar `mod drivers;` em `src/main.rs`

### Onde aplicar

Faca isso no topo de `src/main.rs`, junto dos outros modulos raiz.

### Como localizar o ponto exato

Hoje o arquivo comeca com algo parecido com:

```rust
#![no_std]
#![no_main]
mod app;
```

Adicione logo abaixo:

```rust
mod drivers;
```

### Como fica

```rust
#![no_std]
#![no_main]

mod app;
mod drivers;
```

### O que isso resolve

Sem essa declaracao, os arquivos em `src/drivers/` nao passam a fazer parte do crate.

---

## 3. Criar o arquivo `src/drivers/am2302.rs`

### Onde aplicar

Crie o arquivo:

```text
src/drivers/am2302.rs
```

### O que esse arquivo deve conter neste passo

Neste passo 2, ele deve conter apenas a estrutura base do driver.

Ou seja, o arquivo ainda nao precisa implementar todo o protocolo, mas ja precisa declarar:

- os imports das traits necessarias
- `Am2302Error`
- `Am2302<PIN, D>`
- `new(...)`
- `read(...)`

---

## 4. Importar as traits corretas

### O que o plano quer expressar

O plano sugere:

```rust
Am2302<PIN: OutputPin + InputPin, D: DelayUs>
```

No ecossistema Rust embarcado, isso normalmente significa usar traits do `embedded-hal`.

### Estrutura sugerida

Uma base coerente seria algo assim:

```rust
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
```

### Observacao importante

No plano, o nome aparece como `DelayUs`, mas nas versoes mais novas do `embedded-hal` o equivalente mais comum costuma ser `DelayNs`, com metodos que aceitam nanos, micros e millis.

Entao, neste passo, o mais importante nao e o nome literal do trait do plano.

O mais importante e:

1. existir uma abstracao de delay fina
2. o driver nao depender diretamente de uma API concreta da STM32

---

## 5. Criar `Am2302Error`

### Onde aplicar

Faca isso em `src/drivers/am2302.rs`, antes da struct do driver.

### O que esse enum deve cobrir neste passo

Mesmo antes de implementar o protocolo completo, o enum ja deve nascer cobrindo os erros mais provaveis do AM2302.

Uma forma minima e util seria:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Am2302Error {
    // O sensor nao respondeu no tempo esperado.
    Timeout,

    // Os 40 bits foram lidos, mas o checksum nao bateu.
    Checksum,

    // O protocolo nao bateu com o esperado.
    Protocol,

    // Falha ao dirigir a linha para output ou ao ler o input.
    Pin,
}
```

### Por que isso ja ajuda agora

Mesmo antes do corpo final de `read()`, esse enum ja define como o resto do firmware vai enxergar falhas do sensor.

---

## 6. Criar a struct `Am2302<PIN, D>`

### Onde aplicar

Ainda em `src/drivers/am2302.rs`, logo abaixo de `Am2302Error`.

### Estrutura sugerida

```rust
pub struct Am2302<PIN, D> {
    pin: PIN,
    delay: D,
}
```

### O que cada campo representa

- `pin`
  - o GPIO que sera usado tanto para o start quanto para a leitura dos pulsos

- `delay`
  - a abstracao de temporizacao fina em microssegundos

### Observacao importante

Neste passo, a struct nao precisa armazenar buffer de leitura, timeout configuravel ou estado interno mais sofisticado.

O melhor aqui e manter a menor estrutura correta.

---

## 7. Criar o `impl` com bounds genericos

### Forma sugerida

Uma forma coerente com o plano seria:

```rust
impl<PIN, D> Am2302<PIN, D>
where
    PIN: OutputPin + InputPin,
    D: DelayNs,
{
    // ...
}
```

### Por que isso e importante

Esses bounds explicam de forma explicita o que o driver precisa do hardware:

- controlar a linha em output
- ler a linha em input
- esperar tempos curtos com precisao

---

## 8. Implementar `new(pin, delay)`

### Onde aplicar

Dentro do `impl<PIN, D> Am2302<PIN, D>`.

### Forma direta e pequena

```rust
pub fn new(pin: PIN, delay: D) -> Self {
    Self { pin, delay }
}
```

### O que isso resolve

- padroniza a criacao do driver
- prepara a integracao futura no `main.rs`

---

## 9. Declarar `read()` com a assinatura final

### O que o plano pede

O plano quer:

```rust
read() -> Result<(f32, f32), Am2302Error>
```

### Como declarar neste passo

Neste momento, a melhor estrategia e declarar a funcao com a assinatura final, mesmo que o corpo ainda fique provisoriamente incompleto.

Exemplo de estrutura:

```rust
pub fn read(&mut self) -> Result<(f32, f32), Am2302Error> {
    // Neste passo, a assinatura final ja fica definida.
    // A implementacao completa do protocolo entra no passo 3.
    Err(Am2302Error::Protocol)
}
```

### Observacao importante

Se voce preferir deixar um marcador explicito durante a construcao do passo, tambem pode usar algo conceitualmente assim:

```rust
pub fn read(&mut self) -> Result<(f32, f32), Am2302Error> {
    // TODO: implementar a leitura completa do protocolo no passo 3.
    Err(Am2302Error::Protocol)
}
```

### Por que isso e melhor do que tentar implementar tudo agora

Porque este passo e sobre API e estrutura.

O passo 3 sera justamente o momento de implementar:

- start
- resposta do sensor
- leitura de `40 bits`
- checksum
- conversao para temperatura e umidade

---

## 10. Estrutura completa sugerida para este passo

Uma forma pequena e coerente de deixar `src/drivers/am2302.rs` ao final do passo 2 seria:

```rust
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Am2302Error {
    // O sensor nao respondeu no tempo esperado.
    Timeout,

    // Os bytes lidos nao bateram com o checksum informado.
    Checksum,

    // O fluxo do protocolo nao bateu com o esperado.
    Protocol,

    // Falha ao acessar o GPIO do sensor.
    Pin,
}

pub struct Am2302<PIN, D> {
    pin: PIN,
    delay: D,
}

impl<PIN, D> Am2302<PIN, D>
where
    PIN: OutputPin + InputPin,
    D: DelayNs,
{
    pub fn new(pin: PIN, delay: D) -> Self {
        Self { pin, delay }
    }

    pub fn read(&mut self) -> Result<(f32, f32), Am2302Error> {
        // TODO: implementar a leitura completa do AM2302 no passo 3.
        Err(Am2302Error::Protocol)
    }
}
```

---

## 11. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- start real do AM2302
- espera da resposta `80 us + 80 us`
- leitura dos `40 bits`
- checksum real
- conversao dos bytes para `f32`
- integracao no `main.rs`
- task periodica
- shell `sensors`

Tudo isso entra depois.

Neste passo 2, o sucesso e apenas:

- a pasta `src/drivers/` existe
- o arquivo `src/drivers/am2302.rs` existe
- o driver tem a API publica principal
- o projeto continua compilando

---

## 12. Checklist de validacao

Antes de seguir para o passo 3, valide pelo menos isto:

1. `src/drivers/mod.rs` existe
2. `src/drivers/am2302.rs` existe
3. `mod drivers;` foi declarado em `src/main.rs`
4. `Am2302Error` existe
5. `Am2302<PIN, D>` existe
6. `new(pin, delay)` existe
7. `read()` existe com a assinatura final
8. `cargo check`

### Resultado esperado

Se este passo foi feito corretamente:

- o projeto ganha a estrutura minima do driver
- a API publica do AM2302 ja fica decidida
- o passo 3 pode focar apenas na implementacao do protocolo

---

## Resumo deste passo

O passo 2 da Fase 5 nao e sobre fazer o sensor funcionar ainda.

Ele e sobre criar a estrutura correta do driver antes de lidar com a parte dificil de temporizacao e protocolo.

Ao final deste passo, o projeto deve ter:

- `src/drivers/mod.rs`
- `src/drivers/am2302.rs`
- `Am2302Error`
- `Am2302<PIN, D>`
- `new(pin, delay)`
- `read() -> Result<(f32, f32), Am2302Error>`

Com isso, a Fase 5 passa a ter uma base de driver organizada e pronta para o passo 3, que sera a implementacao real do protocolo do AM2302.
