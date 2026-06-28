## Fase 5 - Passo 1: Estudar o datasheet do AM2302

Este arquivo descreve o primeiro passo da Fase 5.

Objetivo deste passo:

- estudar o datasheet do AM2302 antes de escrever o driver
- confirmar a sequencia de comunicacao e o formato dos 40 bits
- levantar os cuidados eletricos e de montagem para usar o sensor com a STM32 na protoboard

Neste passo, o foco ainda nao e escrever `src/drivers/am2302.rs`.

O foco agora e sair com uma especificacao pratica suficiente para:

- montar o circuito corretamente
- escolher uma estrategia de driver
- evitar erro eletrico logo no primeiro teste em hardware

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 5 define como primeiro passo:

1. estudar o datasheet para confirmar:
   - sequencia de start
   - formato da resposta
   - checksum

Mas, no seu caso, isso nao e suficiente sozinho.

Como voce esta com:

- a placa STM32
- a protoboard
- jumpers
- o AM2302 em maos

este passo tambem precisa responder:

1. como alimentar o sensor
2. como ligar o pino de dados na STM32
3. se precisa resistor
4. se precisa capacitor
5. quais cuidados de tensao e temporizacao devem ser respeitados

---

## Fontes usadas neste passo

Este passo foi escrito com base principalmente em:

- `Documents/Digital+humidity+and+temperature+sensor+AM2302_cópia.pdf`
- `PLANO_TRABALHO.md`

Pontos extraidos do datasheet do AM2302:

- alimentacao: `3.3 V` a `5.5 V`
- barramento digital proprio de fio unico
- start do MCU: linha em nivel baixo por `1 ms` a `10 ms`
- depois o MCU libera a linha e espera `20 us` a `40 us`
- resposta do sensor: `80 us` low + `80 us` high
- dados: `40 bits`
- cada bit comeca com `50 us` low
- bit `0`: high de cerca de `26 us` a `28 us`
- bit `1`: high de cerca de `70 us`
- intervalo entre leituras: maior que `2 s`
- capacitor recomendado: `100 nF` entre `VDD` e `GND`
- resistor de pull-up mostrado no diagrama: `1 kOhm`

---

## 1. O que o datasheet confirma sobre alimentacao

### Faixa de tensao do sensor

O datasheet informa que o AM2302 aceita:

```text
3.3 V a 5.5 V DC
```

Isso e excelente para o seu projeto, porque a STM32 da Nucleo trabalha com `3.3 V` nos GPIOs.

### Recomendacao pratica para este projeto

Para usar com a STM32, a escolha mais segura e recomendada e:

```text
alimentar o AM2302 com 3.3 V
```

### Por que usar 3.3 V aqui

Porque assim:

- o nivel alto do pino `DATA` tambem fica em `3.3 V`
- isso combina naturalmente com os GPIOs da STM32
- voce evita discutir nivel logico de `5 V` no pino de entrada da MCU
- voce simplifica a montagem na protoboard

### Recomendacao objetiva

Para este projeto:

1. use o pino `3V3` da placa STM32 para alimentar o AM2302
2. nao use `5V` para este primeiro teste
3. mantenha `GND` comum entre placa e sensor

---

## 2. O que o datasheet confirma sobre o fio de dados

### Tipo de barramento

O datasheet informa que o AM2302 usa um barramento digital proprio de fio unico.

Importante:

- nao e UART
- nao e I2C
- nao e SPI
- nao e o 1-wire da Dallas/Maxim

O proprio datasheet diz que esse protocolo e diferente do 1-wire da Dallas.

### O que isso implica para o driver

O pino da MCU precisa conseguir:

1. dirigir a linha em output durante o start
2. liberar a linha
3. ler a resposta como input
4. medir pulsos de microssegundos

Entao, no driver, o pino escolhido precisara ser tratado como GPIO bidirecional por software.

---

## 3. Sequencia eletrica basica da comunicacao

O datasheet confirma esta ordem:

1. linha ociosa fica em nivel alto
2. MCU puxa a linha para low por `1 ms` a `10 ms`
3. MCU libera a linha
4. MCU espera `20 us` a `40 us`
5. sensor responde com `80 us` low
6. sensor responde com `80 us` high
7. sensor envia `40 bits`

### Resumo operacional

Em termos de implementacao, a conversa e esta:

```text
MCU start -> sensor responde -> sensor envia 40 bits -> MCU valida checksum
```

### Constantes uteis para o driver

Quando voce for implementar o driver depois, estas constantes fazem sentido:

```rust
// Tempo minimo de start em nivel baixo.
const START_LOW_MS: u32 = 2;

// Tempo de espera apos liberar a linha antes da resposta do sensor.
const START_RELEASE_WAIT_US: u32 = 30;

// Numero total de bits transmitidos pelo sensor.
const FRAME_BITS: usize = 40;

// Intervalo minimo entre leituras completas do sensor.
const MIN_SAMPLE_INTERVAL_MS: u32 = 2000;
```

Esses valores ainda nao precisam entrar no codigo agora.

Mas eles ajudam a organizar mentalmente o driver que vira no passo 2 e 3.

---

## 4. Formato dos 40 bits

O datasheet confirma que o frame enviado pelo AM2302 tem:

```text
16 bits de umidade + 16 bits de temperatura + 8 bits de checksum
```

### Ordem logica dos bytes

Uma forma util de enxergar o frame e:

```text
byte0  byte1  byte2  byte3  byte4
RH_H   RH_L   T_H    T_L    CHK
```

### Conversao de umidade

O datasheet mostra que a umidade vem em decimos.

Exemplo:

```text
652 -> 65.2 %RH
```

### Conversao de temperatura

A temperatura tambem vem em decimos.

Exemplo:

```text
351 -> 35.1 C
```

### Temperatura negativa

O datasheet informa que, quando o bit mais alto do campo de temperatura for `1`, o valor representa temperatura negativa.

Entao o driver precisara tratar explicitamente esse caso.

### Estrutura util para lembrar

```rust
// bytes[0] e bytes[1] formam a umidade em decimos.
// bytes[2] e bytes[3] formam a temperatura em decimos.
// bytes[4] e o checksum.
let bytes = [rh_hi, rh_lo, t_hi, t_lo, checksum];
```

---

## 5. Regra do checksum

O datasheet define o checksum assim:

```text
checksum = 8 bits menos significativos da soma dos 4 primeiros bytes
```

Ou seja:

```text
CHK = (byte0 + byte1 + byte2 + byte3) & 0xFF
```

### Forma direta de pensar

No driver, a validacao vai ficar conceitualmente assim:

```rust
// Soma os 4 primeiros bytes e compara com o checksum recebido.
let expected = bytes[0]
    .wrapping_add(bytes[1])
    .wrapping_add(bytes[2])
    .wrapping_add(bytes[3]);

// Se o checksum nao bater, a leitura deve falhar.
if expected != bytes[4] {
    // retorna erro de checksum
}
```

### O que isso evita

Essa verificacao e importante para detectar:

- ruido na linha de dados
- temporizacao errada do driver
- sensor mal conectado
- leitura parcialmente corrompida

---

## 6. Temporizacao dos bits

O datasheet confirma que cada bit transmitido pelo sensor tem esta estrutura:

1. `50 us` em low
2. depois um high que define `0` ou `1`

### Valores relevantes

- bit `0` -> high de cerca de `26 us` a `28 us`
- bit `1` -> high de cerca de `70 us`

### O que isso implica para o driver

O driver nao precisa medir cada pulso com precisao absoluta de laboratorio, mas precisa distinguir bem dois grupos:

- high curto -> `0`
- high longo -> `1`

### Regra pratica de decodificacao

Uma estrategia simples depois sera:

```rust
// Se o tempo em high passou de um limiar intermediario, interpreta como 1.
// Se nao passou, interpreta como 0.
if high_time_us > 50 {
    // bit = 1
} else {
    // bit = 0
}
```

O valor exato do limiar pode ser ajustado depois, mas o importante neste passo e entender a diferenca clara entre os dois pulsos.

---

## 7. Intervalo minimo entre leituras

O datasheet informa que o intervalo do processo completo deve ser maior que `2 segundos`.

### O que isso implica para a task futura

Quando voce integrar o sensor em uma task periodica, a task nao deve ficar lendo em loop rapido.

### Recomendacao para este projeto

Use algo na faixa de:

```text
2 s a 5 s entre leituras
```

Isso ja esta alinhado com o `PLANO_TRABALHO.md`.

---

## 8. Montagem eletrica na protoboard

### GPIO recomendado para o `DATA`

Para este projeto, a recomendacao principal e usar:

```text
PB6
```

### Por que `PB6` e uma boa escolha aqui

No estado atual do firmware, `PB6` e uma escolha boa porque:

- nao esta sendo usado pelas funcionalidades ja implementadas
- e um GPIO digital comum, adequado para alternar entre entrada e saida no driver
- tende a ser uma escolha pratica de cabeamento na Nucleo + protoboard
- evita conflito com os pinos ja ocupados hoje no projeto

### Como tratar essa recomendacao na bancada

Monte pensando assim:

```text
DATA do AM2302 -> PB6 da STM32
```

### Como esse pino aparece na sua placa

Na sua Nucleo, esse mesmo pino aparece na serigrafia como:

```text
PWM/CS/D10
```

Entao, para a sua montagem real de bancada, a leitura pratica fica assim:

```text
DATA do AM2302 -> PWM/CS/D10 na placa
```

e, no codigo, continua valendo:

```text
PB6
```

### Resumo importante

- nome no firmware: `PB6`
- nome visto na placa: `PWM/CS/D10`
- eletricamente, e o mesmo pino

### Observacao importante

Antes de fechar o jumper, confira no silk screen da sua placa ou no pinout da Nucleo qual pino fisico do header corresponde a `PB6`.

Ou seja, a recomendacao aqui e:

1. usar o sinal logico `PB6`
2. localizar no header da sua placa onde esse `PB6` esta exposto
3. so entao ligar o fio `DATA` do AM2302 nesse ponto

### Se `PB6` estiver inconveniente no seu header

Se, na sua placa ou na sua montagem da protoboard, `PB6` ficar ruim de acessar, a segunda linha de raciocinio continua sendo:

- escolher outro GPIO livre
- que nao esteja preso a UART, I2C, ADC, PWM, LED ou botao
- e que seja simples de usar como entrada e saida digital

### Ligacao minima recomendada

Para o primeiro teste, monte assim:

1. `VDD` do AM2302 -> `3V3` da STM32
2. `GND` do AM2302 -> `GND` da STM32
3. `DATA` do AM2302 -> `PB6` da STM32
4. resistor de pull-up entre `DATA` e `3V3`
5. capacitor `100 nF` entre `VDD` e `GND`, perto do sensor

### O que o datasheet mostra

O diagrama do datasheet mostra explicitamente:

- um resistor de `1 kOhm` entre `VDD` e `DATA`
- um capacitor opcional de `100 nF` entre `VDD` e `GND`

### Esquema textual da montagem

```text
STM32 3V3  -------------------- VDD do AM2302
STM32 GND  -------------------- GND do AM2302
STM32 PWM/CS/D10 (PB6) -------- DATA do AM2302
                 |
                 +---- resistor 1 kOhm ---- 3V3

// Opcional, mas recomendado pelo datasheet:
// capacitor 100 nF entre VDD e GND perto do sensor.
```

### Recomendacao pratica de protoboard

Na protoboard:

1. leve `3V3` e `GND` da Nucleo para os trilhos laterais
2. coloque o sensor de forma que os pinos nao fechem curto entre si
3. coloque o resistor de pull-up entre a linha do `DATA` e a linha de `3V3`
4. coloque o capacitor `100 nF` entre `VDD` e `GND` o mais perto possivel do sensor
5. mantenha os fios curtos no primeiro teste

---

## 9. Como identificar os fios ou pinos do seu sensor

### Se o seu AM2302 for o modelo com 3 fios

O datasheet mostra:

- fio vermelho -> `VDD`
- fio amarelo -> `DATA`
- fio preto -> `GND`

### Se o seu AM2302 for o encapsulamento de 4 pinos

O datasheet apresenta um encapsulamento padrao de 4 pinos.

Na pratica, para esse tipo de sensor, a identificacao mais segura e:

1. olhar a frente do sensor com a grade voltada para voce
2. confirmar a ordem exata no desenho do datasheet
3. so entao inserir na protoboard

### Cuidado importante

Nao assuma a ordem dos pinos apenas por memoria.

Antes de energizar:

1. confirme se o seu sensor e o modelo com fios ou o de 4 pinos
2. confira visualmente a orientacao
3. confirme qual pino e `VDD`, qual e `DATA` e qual e `GND`

### Observacao pratica

Se o seu sensor for um DHT22/AM2302 bare de 4 pinos, a pinagem comum em montagens hobby costuma ser:

```text
1 = VDD
2 = DATA
3 = NC
4 = GND
```

Mas, como ha variacoes de modulo e representacao grafica ruim em alguns materiais, confirme visualmente com o componente em maos antes de ligar energia.

---

## 10. Qual GPIO livre usar na STM32

### O que evitar neste projeto

Pelos pinos ja usados no firmware atual, evite reutilizar:

- `PA5` -> LED
- `PC13` -> botao
- `PA3/PA2` -> LPUART1
- `PB8/PB9` -> I2C1
- `PC0` -> PWM
- `PA7` -> ADC

### Criterio para o pino do AM2302

Escolha um GPIO livre que:

1. esteja acessivel no header da placa
2. possa ser usado como entrada e saida digital comum
3. nao esteja preso a alguma funcao critica ja usada pelo projeto
4. seja facil de cabeamento na protoboard

### Recomendacao pratica deste passo

Neste passo 1, ainda nao feche o pino no codigo.

Mas ja deixe separado um GPIO livre fisicamente acessivel no header da placa para o teste do AM2302.

---

## 11. Cuidados eletricos importantes para nao errar na bancada

### 1. Use `3.3 V` no primeiro teste

Mesmo que o sensor aceite ate `5.5 V`, para este projeto o melhor e usar `3.3 V`.

### 2. Nao esqueça o pull-up em `DATA`

Sem pull-up, a linha pode ficar instavel e o sensor pode parecer morto ou sempre retornar high.

### 3. Aguarde apos energizar

O datasheet diz para nao enviar instrucao ao sensor dentro de `1 segundo` apos energizar.

### 4. Nao leia rapido demais

Mantenha o intervalo entre leituras maior que `2 segundos`.

### 5. Mantenha os fios curtos no primeiro teste

O datasheet menciona que a qualidade do fio afeta a comunicacao.

Entao, antes de testar distancias grandes:

- use jumpers curtos
- mantenha a montagem simples
- teste primeiro na protoboard perto da placa

### 6. Cuidado com inversao de pinos

O erro mais perigoso na bancada aqui e ligar:

- `VDD` em `GND`
- `GND` em `3V3`
- `DATA` no pino errado do sensor

Antes de ligar a placa:

1. revise a orientacao fisica do sensor
2. revise os fios
3. revise o resistor de pull-up

---

## 12. O que este passo define para os proximos

Depois deste estudo, o driver da Fase 5 deve nascer assumindo:

1. alimentacao em `3.3 V`
2. `DATA` com pull-up para `3.3 V`
3. start do MCU em low por cerca de `1 ms` a `10 ms`
4. espera de `20 us` a `40 us`
5. resposta do sensor em `80 us low + 80 us high`
6. leitura de `40 bits`
7. checksum por soma dos 4 primeiros bytes
8. intervalo minimo de mais de `2 s` entre leituras

---

## 13. Checklist de bancada antes de partir para o driver

Antes de seguir para o passo 2 da Fase 5, confirme isto fisicamente:

1. voce identificou se seu sensor e de 3 fios ou 4 pinos
2. voce sabe onde esta `VDD`
3. voce sabe onde esta `DATA`
4. voce sabe onde esta `GND`
5. voce separou um resistor de pull-up
6. voce separou um capacitor `100 nF` se tiver disponivel
7. voce escolheu um GPIO livre da STM32 para o fio `DATA`
8. voce decidiu alimentar com `3.3 V`

---

## Resumo deste passo

O passo 1 da Fase 5 nao e apenas leitura teorica do datasheet.

Para o seu caso, ele tambem fecha a parte de preparacao eletrica da bancada.

As decisoes praticas mais importantes daqui sao:

- alimentar o AM2302 com `3.3 V`
- usar `GND` comum com a STM32
- ligar `DATA` em um GPIO livre
- colocar um pull-up entre `DATA` e `3.3 V`
- se possivel, colocar `100 nF` entre `VDD` e `GND`
- respeitar `1 s` apos energizar antes da primeira leitura
- respeitar mais de `2 s` entre leituras

Com isso, o proximo passo pode ir para `src/drivers/am2302.rs` com base tecnica suficiente e com muito menos risco de erro de montagem.
