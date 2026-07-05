## Fase 6 - Passos 1 e 2: verificacao eletrica e escolha de pinos para o HC-SR04

Este documento fecha os passos 1 e 2 da Fase 6:

- confirmar a seguranca eletrica do `HC-SR04`
- escolher pinos e recursos de hardware sem quebrar o esquema atual

O objetivo aqui e manter o `AM2302` e o `ADXL345` no esquema eletrico e no firmware ao mesmo tempo.

---

## 1. Fontes usadas nesta verificacao

- `Documents/HC-SR04 (1).PDF`
- `Documents/stm32g474re.pdf`
- `src/main.rs`
- `PLANO_TRABALHO.md`

---

## 2. Requisitos eletricos do HC-SR04

Pelo documento consultado do `HC-SR04`:

- alimentacao do modulo: `+5 V`
- corrente de trabalho: cerca de `15 mA`
- `TRIG`: entrada digital; precisa de pulso `HIGH` de `10 us`
- `ECHO`: saida digital que vai para `HIGH` durante o tempo de voo do pulso

Ponto critico:

- o documento consultado trata o `ECHO` como saida em `5 V`
- portanto, ligar `ECHO` em pino errado do STM32 pode danificar o microcontrolador

Conclusao eletrica imediata:

- `TRIG` e seguro saindo do STM32 para o sensor
- `ECHO` precisa ir para um pino realmente `5 V tolerant`, ou entao passar por divisor resistivo/level shifter

---

## 3. Restricoes eletricas do STM32G474RE

O `STM32G474RE` trabalha com `VDD/VDDA` entre `1.71 V` e `3.6 V`.

No datasheet, os pinos se dividem em:

- `FT`: pinos `5 V tolerant`
- `TT`: pinos tolerantes apenas ate a faixa normal de alimentacao do MCU

Para este projeto isso significa:

- pino `FT`: pode receber o `ECHO` em `5 V`, desde que os `pull-up/pull-down` internos nao sejam usados
- pino `TT`: nao deve receber `5 V` direto

Observacao importante do datasheet:

- para tensoes acima de `VDD + 0.3 V`, os resistores internos de `pull-up` e `pull-down` devem ficar desabilitados

Entao, para o `ECHO`:

- se o pino escolhido for `FT`, a ligacao pode ser direta
- se o pino escolhido for `TT`, use divisor resistivo

---

## 4. Pinos ja ocupados no firmware atual

Pelo `src/main.rs`, o firmware atual ja usa:

| Pino | Uso atual | Recurso |
|---|---|---|
| `PA5` | LED | GPIO |
| `PA7` | ADC externo | `ADC2` |
| `PA3` | UART RX | `LPUART1` |
| `PA2` | UART TX | `LPUART1` |
| `PC13` | botao | `EXTI13` |
| `PC0` | PWM | `TIM1_CH1` |
| `PB8` | I2C SCL | `I2C1` |
| `PB9` | I2C SDA | `I2C1` |
| `PC9` | habilitacao do ADXL345 | GPIO |
| `PB6` | `AM2302` | `TIM4_CH1` + `DMA1_CH5` |

Isso gera duas restricoes fortes para o `HC-SR04`:

- nao reutilizar `PB6`
- nao reutilizar `TIM4`, porque ele ja esta comprometido com o `AM2302`

Essa segunda restricao e importante: mesmo que outro canal do `TIM4` pareca livre no datasheet, na arquitetura atual isso criaria acoplamento indevido entre os dois sensores.

---

## 5. Analise dos pinos candidatos para o ECHO

Como a placa alvo e `STM32G474RETx`, o encapsulamento relevante e `LQFP64`.

Precisamos de um pino que seja ao mesmo tempo:

- livre no firmware atual
- exposto no encapsulamento usado
- `5 V tolerant`
- ligado a um timer utilizavel para `input capture`

### 5.1. Candidatos bons

| Pino | Tipo eletrico | Timer/canal | Situacao |
|---|---|---|---|
| `PC6` | `FT_f` | `TIM3_CH1` | livre, mas ruim de localizar na serigrafia da Nucleo |
| `PC7` | `FT_f` | `TIM3_CH2` | livre e recomendado |
| `PC8` | `FT_f` | `TIM3_CH3` | livre, mas menos necessario |

Todos esses pinos sao bons eletricamente para receber `ECHO` em `5 V` sem divisor, desde que configurados sem `pull-up`/`pull-down` interno.

### 5.2. Candidatos descartados ou menos indicados

| Pino | Motivo |
|---|---|
| `PB6` | ja usado pelo `AM2302` e pelo `TIM4_CH1` |
| `PB7` | embora seja `FT`, tambem cai no `TIM4`; melhor evitar misturar com o `AM2302` |
| `PB8`/`PB9` | ja ocupados pelo `I2C1` do `ADXL345` |
| `PA11`/`PA12` | poderiam capturar `TIM4`, mas cairiam no mesmo problema de compartilhar `TIM4`; alem disso sao pinos tradicionalmente sensiveis por USB |
| `PB4`/`PA15` | pinos de debug/JTAG; melhor nao consumir se ha opcoes mais limpas |
| pinos `TT` livres como `PA6`, `PB1`, `PB10`, `PB11`, `PC1`, `PC3`, `PC5` | exigiriam divisor resistivo para o `ECHO` |

Conclusao para `ECHO`:

- **recomendacao principal final: `PC7` como `TIM3_CH2`**
- `PC7` aparece na serigrafia Arduino da Nucleo como **`D9`**
- `PC6` continua eletricamente valido, mas ficou menos pratico para a montagem em bancada

---

## 6. Analise do pino TRIG

O `TRIG` nao e o problema de seguranca.

Ele e uma saida do STM32 indo para uma entrada do sensor, entao o requisito principal e ser um GPIO livre.

Pino recomendado:

- **`PA6` como GPIO output comum**
- na serigrafia Arduino da Nucleo, `PA6` aparece como **`D12`**

Motivos:

- esta livre no firmware atual
- nao conflita com `AM2302`, `ADXL345`, UART, PWM ou ADC atual
- nao consome pino de debug
- nao depende de recurso de timer para cumprir a funcao de `TRIG`

Observacao funcional:

- eletricamente, `3.3 V` saindo do STM32 para o `TRIG` e seguro
- o documento consultado do `HC-SR04` nao fixa claramente o limiar logico minimo do `TRIG`
- na pratica, muitos modulos aceitam `3.3 V` no `TRIG`, mas isso deve ser confirmado em bancada

Se o modulo usado nao disparar de forma confiavel com `3.3 V` no `TRIG`, a correcao deve ser:

- transistor ou level shifter para elevar o `TRIG` a `5 V`

Nao e necessario divisor resistivo no `TRIG`.

---

## 7. Recomendacao final de pinagem

### Opcao recomendada final

| Sinal HC-SR04 | Pino STM32 | Observacao |
|---|---|---|
| `TRIG` | `PA6` | GPIO output, serigrafia `D12` |
| `ECHO` | `PC7` | `TIM3_CH2`, `FT_f`, serigrafia `D9` |

### Opcao reserva tecnica

| Sinal HC-SR04 | Pino STM32 | Observacao |
|---|---|---|
| `TRIG` | `PA6` | GPIO output, serigrafia `D12` |
| `ECHO` | `PC6` | `TIM3_CH1`, `FT_f`, mas sem identificacao simples no conector Arduino |

---

## 8. Ligacao eletrica recomendada

### 8.1. Montagem final recomendada na Nucleo

| HC-SR04 | Ligacao |
|---|---|
| `VCC` | `5 V` da placa |
| `GND` | `GND` comum da placa |
| `TRIG` | `D12` (`PA6`) |
| `ECHO` | divisor resistivo -> `D9` (`PC7`) |

### 8.2. Divisor resistivo escolhido para o ECHO

- resistor superior: `2.2 kOhm` entre `ECHO` do sensor e o ponto central
- resistor inferior: `2.2 kOhm + 2.2 kOhm` em serie entre o ponto central e `GND`
- o ponto central vai para `D9` (`PC7`)

Topologia final:

```text
HC-SR04 ECHO ----[2.2k]----+---- D9 / PC7
                           |
                        [2.2k]
                           |
                        [2.2k]
                           |
                          GND
```

Valor esperado no ponto central quando `ECHO = 5 V`:

- `Vout = 5 * 4.4 / (2.2 + 4.4) = 3.33 V`

### 8.3. Condicoes obrigatorias para essa ligacao

- `PC7` deve ser configurado como entrada/AF sem `pull-up` nem `pull-down`
- o terra do `HC-SR04` precisa ser o mesmo terra do STM32
- o `HC-SR04` deve continuar alimentado em `5 V`, nao em `3.3 V`
- o fio de `D9` deve ir ao ponto central do divisor, nao direto ao `ECHO`

### 8.4. Se por qualquer motivo o ECHO for ligado em pino TT

Ai passa a ser obrigatorio usar divisor resistivo.

Exemplo simples:

- resistor superior: `10 kOhm` entre `ECHO` e o pino do MCU
- resistor inferior: `20 kOhm` entre o pino do MCU e `GND`

Isso reduz `5 V` para aproximadamente `3.33 V`.

---

## 9. Impacto sobre o esquema atual

Mantendo o outro sensor no esquema, o resultado fica consistente:

| Bloco | Pinos mantidos |
|---|---|
| `AM2302` | `PB6` + `TIM4_CH1` + `DMA1_CH5` |
| `ADXL345` | `PB8/PB9` (`I2C1`) + `PC9` |
| `HC-SR04` | `PA6` / `D12` (`TRIG`) + `PC7` / `D9` (`ECHO/TIM3_CH2`) |

Isso evita:

- conflito de pino com o `AM2302`
- conflito de timer com o `AM2302`
- conflito com o barramento `I2C1` do `ADXL345`
- uso desnecessario de pinos de debug

---

## 10. Checklist de seguranca eletrica

- `HC-SR04 VCC` em `5 V`
- `GND` comum entre sensor e placa
- `ECHO` em `D9` / `PC7` com divisor resistivo montado corretamente
- `pull-up/pull-down` internos desabilitados no pino do `ECHO`
- nao reutilizar `TIM4`, porque ele ja pertence ao `AM2302`
- manter cabos curtos e conexoes firmes
- se houver instabilidade de alimentacao, adicionar desacoplamento proximo ao sensor, por exemplo `100 nF` + `10 uF`

---

## 11. Decisao final para a Fase 6

Para seguir com seguranca e sem conflito com o hardware ja existente:

- `TRIG = PA6 = D12`
- `ECHO = PC7 = D9`
- `timer do ECHO = TIM3_CH2`
- `divisor resistivo no ECHO = obrigatorio por decisao de bancada`
- `divisor escolhido = 2.2k em cima e 4.4k embaixo`
- `TIM4 = reservado exclusivamente para o AM2302`

Essa e a melhor base para implementar o passo seguinte do driver do `HC-SR04`.
