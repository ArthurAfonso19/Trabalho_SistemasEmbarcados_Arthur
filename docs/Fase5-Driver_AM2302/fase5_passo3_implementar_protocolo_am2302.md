## Fase 5 - Passo 3: Implementar o protocolo do `AM2302`

Este arquivo descreve o terceiro passo da Fase 5.

Objetivo deste passo:

- implementar a leitura real do protocolo do AM2302 dentro de `src/drivers/am2302.rs`
- executar o start do sensor
- esperar a resposta do sensor
- ler os `40 bits`
- validar checksum
- converter os bytes em temperatura e umidade

Agora a Fase 5 deixa de ser apenas estrutura de driver.

Neste passo, o metodo `read()` deixa de ser um stub e passa a executar o protocolo de verdade.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 5 define como terceiro passo:

1. implementar:
   - start: pino baixo por `1 ms` a `2 ms`
   - esperar resposta do sensor
   - ler `40 bits` com temporizacao
   - validar checksum
   - converter para temperatura e umidade

Entao este passo ainda nao pede:

- integrar no `main.rs`
- criar task periodica
- expor `sensors` na shell

Essas partes entram nos passos seguintes da Fase 5.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/drivers/am2302.rs`

Opcionalmente, se voce preferir organizar pequenas constantes ou helpers privados, tudo ainda pode continuar no mesmo arquivo.

No estado atual do projeto, essa e a menor mudanca correta.

---

## Visao geral da ideia

No fim do passo 2, o driver ja possui:

- `Am2302Error`
- `Am2302<PIN, D>`
- `new(pin, delay)`
- `read()` com a assinatura final

Mas o corpo de `read()` ainda nao executa nada de real.

Agora o fluxo do metodo `read()` deve passar a ser, conceitualmente:

1. garantir o start do sensor
2. esperar a sequencia de resposta
3. ler os `40 bits`
4. agrupar em `5 bytes`
5. validar o checksum
6. converter para `(temperatura, umidade)`
7. retornar `Ok((temp_c, humidity_rh))`

---

## Escolha simples para este passo

Para este projeto, a forma mais segura e implementar o protocolo em funcoes pequenas privadas dentro de `src/drivers/am2302.rs`.

Em vez de colocar tudo cru dentro de `read()`, vale dividir em helpers como:

```rust
// Executa o start do protocolo e espera a resposta inicial do sensor.
fn begin_signal(&mut self) -> Result<(), Am2302Error>

// Le um unico bit do barramento do AM2302.
fn read_bit(&mut self) -> Result<u8, Am2302Error>

// Le os 40 bits completos e devolve 5 bytes.
fn read_frame(&mut self) -> Result<[u8; 5], Am2302Error>

// Valida o checksum do frame recebido.
fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302Error>

// Converte os bytes em temperatura e umidade.
fn decode_measurements(bytes: &[u8; 5]) -> Result<(f32, f32), Am2302Error>
```

Essa divisao e boa porque:

- reduz o risco de erro em um metodo gigante
- facilita testar e depurar cada etapa mentalmente
- deixa `read()` mais legivel

---

## 1. Substituir o stub de `read()` pelo fluxo real

### Onde aplicar

Faca isso em `src/drivers/am2302.rs`, dentro do `impl Am2302<PIN, D>`.

### O que existe hoje

No estado atual do projeto, `read()` ainda esta conceitualmente assim:

```rust
pub fn read(&mut self) -> Result<(f32, f32), Am2302Error> {
    Err(Am2302Error::Protocol)
}
```

### O que deve virar

O novo `read()` deve passar a coordenar o protocolo inteiro:

```rust
pub fn read(&mut self) -> Result<(f32, f32), Am2302Error> {
    // Executa o start e confirma que o sensor respondeu.
    self.begin_signal()?;

    // Le os 40 bits completos como 5 bytes.
    let bytes = self.read_frame()?;

    // Garante que o frame nao veio corrompido.
    Self::validate_checksum(&bytes)?;

    // Converte os bytes crus em temperatura e umidade.
    Self::decode_measurements(&bytes)
}
```

### Observacao importante

O importante neste passo nao e copiar esse esqueleto literalmente, mas manter a ordem logica correta do protocolo.

---

## 2. Implementar o start do AM2302

### O que o datasheet pede

O passo 1 ja levantou do datasheet esta sequencia:

1. linha ociosa em high
2. MCU puxa a linha para low por `1 ms` a `10 ms`
3. MCU libera a linha
4. MCU espera `20 us` a `40 us`
5. sensor responde com `80 us` low
6. sensor responde com `80 us` high

### Onde aplicar

Implemente isso em uma helper privada de `src/drivers/am2302.rs`.

### Estrutura sugerida

```rust
fn begin_signal(&mut self) -> Result<(), Am2302Error> {
    // Coloca a linha em nivel baixo para iniciar o protocolo.
    self.pin.set_low().map_err(|_| Am2302Error::Pin)?;

    // Mantem low por um tempo seguro dentro da janela do datasheet.
    self.delay.delay_ms(2);

    // Libera a linha antes de esperar a resposta do sensor.
    self.pin.set_high().map_err(|_| Am2302Error::Pin)?;

    // Espera a janela curta de liberacao antes da resposta.
    self.delay.delay_us(30);

    // Agora a implementacao precisa confirmar:
    // 1. low de resposta
    // 2. high de preparacao
    // Isso entra nos proximos subitens deste passo.
    Ok(())
}
```

### Implementacao completa recomendada para este passo

No codigo final do passo 3, essa helper nao deve parar no `delay_us(30)`.

Ela ja deve confirmar a resposta do sensor.

Uma forma simples e coerente de fechar essa funcao e:

```rust
fn begin_signal(&mut self) -> Result<(), Am2302Error> {
    // Coloca a linha em nivel baixo para iniciar o protocolo.
    self.pin.set_low().map_err(|_| Am2302Error::Pin)?;

    // Mantem low por um tempo seguro dentro da janela do datasheet.
    self.delay.delay_ms(2);

    // Libera a linha antes de esperar a resposta do sensor.
    self.pin.set_high().map_err(|_| Am2302Error::Pin)?;

    // Espera a janela curta de liberacao antes da resposta.
    self.delay.delay_us(30);

    // Espera a linha cair: resposta low do sensor.
    self.wait_for_level(false, 100)?;

    // Espera a linha subir: resposta high do sensor.
    self.wait_for_level(true, 100)?;

    // Espera terminar a fase high de resposta para iniciar os bits.
    self.wait_for_level(false, 100)?;

    Ok(())
}
```

### O que faltava aqui

Se essa parte nao for implementada por completo, o driver ainda nao fecha a etapa de "esperar resposta do sensor" pedida pelo passo 3.

### Observacao importante

Neste projeto, o driver esta modelado com um unico `PIN: OutputPin + InputPin`.

Entao o pino precisa conseguir ser usado como linha dirigida e linha lida pelo software.

Se o seu tipo concreto exigir troca explicita de modo entre output e input, esse detalhe precisara ser tratado na integracao concreta depois.

Mas, no nivel do passo 3, o protocolo deve ser modelado como:

- MCU dirige o start
- MCU libera e observa a resposta

---

## 3. Esperar a resposta inicial do sensor

### O que precisa ser confirmado

Depois do start, o driver precisa ver:

1. `80 us` low
2. `80 us` high

### Estrategia simples

Como este passo e sobre protocolo e nao sobre precisao perfeita de laboratorio, uma estrategia aceitavel e usar polling com timeout.

Exemplo conceitual:

```rust
// Espera a linha ir para low dentro de um timeout curto.
self.wait_for_level(false, 100)?;

// Espera a linha voltar para high dentro de um timeout curto.
self.wait_for_level(true, 100)?;

// Espera terminar a fase high de resposta para comecar a leitura dos bits.
self.wait_for_level(false, 100)?;
```

### Helper util

Vale criar uma helper privada assim:

```rust
// Espera a linha atingir o nivel desejado dentro de um timeout em microssegundos.
fn wait_for_level(&mut self, expected_high: bool, timeout_us: u32) -> Result<(), Am2302Error>
```

### Comportamento esperado dessa helper

- se a linha atingir o nivel esperado, retorna `Ok(())`
- se o tempo passar e isso nao acontecer, retorna `Err(Am2302Error::Timeout)`

### Implementacao completa recomendada

Uma versao pequena e direta para este projeto e:

```rust
fn wait_for_level(&mut self, expected_high: bool, timeout_us: u32) -> Result<(), Am2302Error> {
    for _ in 0..timeout_us {
        // Le o nivel atual da linha de dados.
        let is_high = self.pin.is_high().map_err(|_| Am2302Error::Pin)?;

        // Se a linha chegou no nivel esperado, conclui com sucesso.
        if is_high == expected_high {
            return Ok(());
        }

        // Espera 1 us antes de tentar de novo.
        self.delay.delay_us(1);
    }

    Err(Am2302Error::Timeout)
}
```

### Observacao importante

No seu arquivo atual, essa funcao nao pode ficar apenas com a assinatura.

Sem corpo, o passo 3 permanece incompleto.

---

## 4. Ler um unico bit do sensor

### O que o datasheet descreve

Cada bit tem:

1. `50 us` low
2. depois um high curto ou longo

Interpretacao:

- high curto -> `0`
- high longo -> `1`

### Estrategia recomendada

Uma helper privada pequena para um unico bit e a forma mais clara:

```rust
// Le um bit do frame do AM2302.
fn read_bit(&mut self) -> Result<u8, Am2302Error>
```

### Fluxo conceitual

```rust
fn read_bit(&mut self) -> Result<u8, Am2302Error> {
    // Espera a fase low obrigatoria do bit.
    self.wait_for_level(true, 100)?;

    // Mede quanto tempo a linha permanece em high.
    let high_time_us = self.measure_high_pulse_us(100)?;

    // Decide entre 0 e 1 usando um limiar intermediario.
    if high_time_us > 50 {
        Ok(1)
    } else {
        Ok(0)
    }
}
```

### Observacao importante

O numero `50` aqui nao vem como regra sagrada do datasheet.

Ele e um limiar pratico entre:

- `26 us` a `28 us`
- `70 us`

Ou seja, ele ajuda a separar bem `0` e `1`.

---

## 5. Medir a duracao do pulso high

### O que esta sendo medido

Para decidir se o bit e `0` ou `1`, o driver precisa saber quanto tempo a linha ficou em high.

### Helper sugerida

```rust
// Mede a duracao do pulso em high, em microssegundos.
fn measure_high_pulse_us(&mut self, timeout_us: u32) -> Result<u32, Am2302Error>
```

### Estrategia simples neste projeto

Como o plano ja reconhece que a leitura e bloqueante no microssegundo, a solucao mais direta aqui e:

1. entrar em polling apertado
2. contar tempo em microssegundos com o delay/clock disponivel
3. parar quando a linha sair de high

### Observacao importante

Neste passo, a documentacao deve assumir uma implementacao bloqueante curta.

Isso esta alinhado com o plano, que ja aceitou que o AM2302 exige temporizacao fina e bloqueante no momento da leitura.

### Implementacao completa recomendada

Uma versao pequena e suficiente para este passo seria:

```rust
fn measure_high_pulse_us(&mut self, timeout_us: u32) -> Result<u32, Am2302Error> {
    let mut elapsed_us = 0;

    while elapsed_us < timeout_us {
        // Enquanto a linha continuar em high, soma tempo.
        let is_high = self.pin.is_high().map_err(|_| Am2302Error::Pin)?;

        // Quando a linha cair, a largura do pulso foi medida.
        if !is_high {
            return Ok(elapsed_us);
        }

        self.delay.delay_us(1);
        elapsed_us += 1;
    }

    Err(Am2302Error::Timeout)
}
```

### O que faltava aqui

Sem essa helper implementada, `read_bit()` ainda nao consegue distinguir `0` de `1`, entao a leitura do frame continua incompleta.

---

## 6. Ler o frame completo de 40 bits

### O que precisa sair dessa etapa

Ao final da leitura, o driver deve montar:

```rust
[u8; 5]
```

### Estrategia recomendada

Use dois loops simples:

1. loop de `0..40` para ler bit por bit
2. agrupamento em `5` bytes

### Estrutura sugerida

```rust
fn read_frame(&mut self) -> Result<[u8; 5], Am2302Error> {
    let mut bytes = [0u8; 5];

    for bit_index in 0..40 {
        // Le um bit do protocolo.
        let bit = self.read_bit()?;

        // Descobre em qual byte esse bit entra.
        let byte_index = bit_index / 8;

        // Desloca o byte atual para a esquerda e injeta o novo bit.
        bytes[byte_index] = (bytes[byte_index] << 1) | bit;
    }

    Ok(bytes)
}
```

### Observacao importante

Essa abordagem funciona bem porque o AM2302 envia os bits em ordem sequencial, e o frame pode ser montado naturalmente byte a byte.

---

## 7. Validar o checksum

### Regra do datasheet

O passo 1 ja confirmou:

```text
checksum = soma dos 4 primeiros bytes, truncada para 8 bits
```

### Helper sugerida

```rust
// Confere se o frame recebido bate com o checksum informado pelo sensor.
fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302Error>
```

### Estrutura simples

```rust
fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302Error> {
    // Recalcula o checksum esperado a partir dos 4 primeiros bytes.
    let expected = bytes[0]
        .wrapping_add(bytes[1])
        .wrapping_add(bytes[2])
        .wrapping_add(bytes[3]);

    // Se nao bater, a leitura deve falhar.
    if expected != bytes[4] {
        return Err(Am2302Error::Checksum);
    }

    Ok(())
}
```

---

## 8. Converter os bytes em temperatura e umidade

### O que o frame representa

O frame tem esta estrutura:

```text
byte0  byte1  byte2  byte3  byte4
RH_H   RH_L   T_H    T_L    CHK
```

### Conversao da umidade

Uma forma simples e:

```rust
let raw_humidity = u16::from(bytes[0]) << 8 | u16::from(bytes[1]);
let humidity_rh = raw_humidity as f32 / 10.0;
```

### Conversao da temperatura

O ponto importante aqui e tratar o bit de sinal.

Exemplo conceitual:

```rust
let raw_temperature = u16::from(bytes[2]) << 8 | u16::from(bytes[3]);

// O bit mais alto indica temperatura negativa.
let negative = (raw_temperature & 0x8000) != 0;

// Remove o bit de sinal antes de converter o valor em decimos.
let magnitude = raw_temperature & 0x7FFF;

let mut temperature_c = magnitude as f32 / 10.0;

if negative {
    temperature_c = -temperature_c;
}
```

### Helper sugerida

```rust
// Converte o frame bruto em (temperatura_c, umidade_rh).
fn decode_measurements(bytes: &[u8; 5]) -> Result<(f32, f32), Am2302Error>
```

### Estrutura recomendada

```rust
fn decode_measurements(bytes: &[u8; 5]) -> Result<(f32, f32), Am2302Error> {
    // Converte a umidade em decimos para %RH.
    let raw_humidity = u16::from(bytes[0]) << 8 | u16::from(bytes[1]);
    let humidity_rh = raw_humidity as f32 / 10.0;

    // Converte a temperatura, tratando o bit de sinal.
    let raw_temperature = u16::from(bytes[2]) << 8 | u16::from(bytes[3]);
    let negative = (raw_temperature & 0x8000) != 0;
    let magnitude = raw_temperature & 0x7FFF;

    let mut temperature_c = magnitude as f32 / 10.0;
    if negative {
        temperature_c = -temperature_c;
    }

    Ok((temperature_c, humidity_rh))
}
```

### Implementacao corrigida para os bytes de 16 bits

Ao montar `raw_humidity` e `raw_temperature`, converta cada byte para `u16` antes do shift e do `|`.

Ou seja, a forma correta e:

```rust
fn decode_measurements(bytes: &[u8; 5]) -> Result<(f32, f32), Am2302Error> {
    // Converte a umidade em decimos para %RH.
    let raw_humidity = (u16::from(bytes[0]) << 8) | u16::from(bytes[1]);
    let humidity_rh = raw_humidity as f32 / 10.0;

    // Converte a temperatura, tratando o bit de sinal.
    let raw_temperature = (u16::from(bytes[2]) << 8) | u16::from(bytes[3]);
    let negative = (raw_temperature & 0x8000) != 0;
    let magnitude = raw_temperature & 0x7FFF;

    let mut temperature_c = magnitude as f32 / 10.0;
    if negative {
        temperature_c = -temperature_c;
    }

    Ok((temperature_c, humidity_rh))
}
```

### O que isso evita

Essa forma evita o erro de tipo:

```text
expected u8, found u16
```

que aparece quando o shift e o `|` sao montados com conversao na ordem errada.

---

## 9. Tratar timeouts e erros de protocolo

### O que precisa ser robusto

Neste passo, o driver ja deve conseguir falhar de forma previsivel quando:

- o sensor nao responde
- a linha fica presa em high
- a linha fica presa em low
- o frame chega incompleto
- o checksum falha

### Regra pratica

Use:

- `Am2302Error::Timeout` quando algo esperado nao acontece a tempo
- `Am2302Error::Checksum` quando os bytes nao batem
- `Am2302Error::Protocol` quando a sequencia recebida nao faz sentido
- `Am2302Error::Pin` quando uma operacao de GPIO falhar

### Exemplo de mapeamento mental

```rust
// Sensor nao respondeu dentro do tempo esperado.
return Err(Am2302Error::Timeout);

// Sequencia recebida nao bateu com o formato esperado.
return Err(Am2302Error::Protocol);
```

---

## 10. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- task periodica no `main.rs`
- leitura automatica a cada `2 s` a `5 s`
- armazenamento global da medicao
- comando `sensors` na shell
- tentativa de otimizar com paralelismo ou abstrações extras

Neste passo 3, o sucesso e apenas:

- `read()` executa o protocolo do AM2302
- o driver retorna `Ok((temperatura, umidade))` quando a leitura e valida
- erros retornam via `Am2302Error`

### O que nao pode continuar faltando ao final deste passo

Para considerar o passo 3 realmente concluido, estas partes nao podem ficar apenas como assinatura ou esqueleto:

- `begin_signal()` sem confirmar a resposta do sensor
- `wait_for_level(...)` sem corpo
- `measure_high_pulse_us(...)` sem corpo
- `decode_measurements(...)` com montagem incorreta de `u16`

---

## 11. Checklist de validacao

Antes de seguir para o passo 4, valide pelo menos isto:

1. `cargo check`
2. `read()` nao e mais um stub
3. o driver executa start + resposta + leitura dos `40 bits`
4. o checksum e validado
5. a conversao para `f32` existe
6. timeouts retornam erro coerente

### Resultado esperado

Se este passo foi feito corretamente:

- o driver do AM2302 ja conversa de verdade com o protocolo
- a API publica do passo 2 passa a ter comportamento real
- o passo 4 pode focar apenas na integracao com `main.rs`

---

## Resumo deste passo

O passo 3 da Fase 5 e a parte central do driver do AM2302.

Ele transforma a estrutura criada no passo 2 em uma implementacao real do protocolo:

- start do sensor
- espera da resposta inicial
- leitura de `40 bits`
- checksum
- conversao para temperatura e umidade

Para manter a implementacao clara e segura, a recomendacao aqui e dividir o trabalho em helpers privados pequenos dentro de `src/drivers/am2302.rs`, em vez de colocar tudo diretamente dentro de `read()`.

Com isso, o projeto fica pronto para o passo 4 da Fase 5, que sera a integracao do driver no `main.rs` e o spawn de uma tarefa periodica de leitura.
