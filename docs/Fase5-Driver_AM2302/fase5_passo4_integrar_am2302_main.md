## Fase 5 - Passo 4: Integrar o `AM2302` no `main.rs`

Este arquivo descreve o quarto passo da Fase 5.

Objetivo deste passo:

- escolher o pino real do sensor no firmware
- instanciar o driver `Am2302` no `main.rs`
- criar uma task periodica de leitura
- validar o funcionamento do sensor antes da integracao com a shell

Neste momento, o foco ainda nao e implementar o comando `sensors`.

O foco agora e colocar o sensor para funcionar no runtime do firmware e observar as medicoes por logs.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 5 define como quarto passo:

1. integrar no `main.rs`:
   - escolher pino
   - instanciar driver
   - spawnar tarefa de leitura periodica

Entao este passo ainda nao pede:

- mostrar os valores pela shell UART
- criar o comando `sensors`
- misturar essa integracao com parser de comandos

Essas partes pertencem ao passo 5 da Fase 5.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/main.rs`
- `src/drivers/am2302.rs`

No estado final que funcionou na placa, a integracao no `main.rs` so ficou confiavel depois de um ajuste real no driver:

- habilitar o contador de ciclos do core no boot
- medir pulsos do protocolo com `DWT::cycle_count()`
- bloquear interrupcoes durante a janela critica de leitura do frame

Entao, neste passo, `main.rs` continua sendo o ponto de integracao com o runtime, mas `src/drivers/am2302.rs` tambem faz parte da implementacao correta.

---

## Resposta curta sobre o teste antes da shell

### Sim, e possivel testar pelo terminal do PowerShell do VS Code

Neste passo, a melhor forma de validar o AM2302 **antes** de criar `sensors` na shell e usar:

```powershell
cargo run
```

No seu projeto, isso grava o firmware na placa e abre os logs RTT no proprio terminal.

Entao, sim: neste passo da para confirmar o funcionamento do AM2302 pelo terminal do PowerShell do VS Code, sem precisar esperar a integracao na shell.

### O que voce deve esperar no terminal

Se a integracao estiver correta, o PowerShell deve mostrar logs periodicos com algo como:

```text
AM2302: temp=24.7 C, humidity=58.3 %RH
```

Ou, em caso de falha:

```text
AM2302 error: Timeout
AM2302 error: Checksum
AM2302 error: Protocol
```

### Por que essa estrategia e boa

- valida o hardware antes de mexer na shell
- simplifica o debug
- separa erro de driver de erro de parser/comando
- segue exatamente a ordem segura do plano

---

## Visao geral da ideia

No fim do passo 3, o projeto ja tem um driver do AM2302 que:

- modela a linha de dados em software
- executa start
- espera a resposta
- le os `40 bits`
- valida checksum
- converte para `(temperatura, umidade)`

Depois da validacao em hardware, ficou claro que isso so funciona de forma estavel quando a temporizacao do frame e medida em ciclos reais de CPU, e nao por contagem aproximada de loops.

Agora falta ligar esse driver ao hardware real do projeto:

1. usar o pino escolhido na bancada
2. criar o objeto concreto da linha do sensor
3. instanciar `Am2302`
4. rodar uma task periodica que faz `read()`
5. logar o resultado no RTT

---

## Escolha simples para este passo

Para este projeto, a opcao mais segura e criar uma task dedicada do sensor em `src/main.rs`, sem envolver shell ainda.

Uma forma simples e coerente e:

1. criar o `Flex` do pino `PB6`
2. criar um pequeno delay concreto
3. instanciar `Am2302::new(...)`
4. fazer leituras a cada `2 s` a `5 s`
5. imprimir no log RTT

Essa escolha e boa porque:

- segue o plano
- facilita testes no PowerShell
- evita misturar protocolo com interface de usuario cedo demais

---

## 1. Escolher o pino real do sensor no firmware

### Qual pino usar

Pelo passo 1 da Fase 5, a recomendacao ficou:

```text
PB6
```

Na sua placa, esse pino aparece como:

```text
PWM/CS/D10
```

### O que isso significa no codigo

No firmware, o pino deve ser tratado como:

```rust
// Pino de dados do AM2302 escolhido para este projeto.
let am2302_data = p.PB6;
```

### O que isso precisa respeitar

- o hardware ja foi montado nesse pino
- o pull-up externo ja esta ligado
- o driver deve usar exatamente o mesmo pino que esta na protoboard

---

## 2. Criar a linha concreta do sensor com `Flex`

### Onde aplicar

Faca isso em `src/main.rs`, dentro da `main()`.

### Como localizar o ponto exato

Procure a regiao onde hoje a `main()` inicializa os perifericos principais, por exemplo:

- botao
- UART
- PWM
- acelerometro

O AM2302 deve entrar nessa mesma fase de inicializacao.

### Estrutura sugerida

```rust
use embassy_stm32::gpio::Flex;
```

E depois, dentro da `main()`:

```rust
// Cria a linha de dados flexivel do AM2302 no PB6.
let am2302_line = Flex::new(p.PB6);
```

### Por que `Flex`

Porque o driver ja foi modelado para controlar a linha por software, alternando entre:

- dirigir low
- liberar a linha
- ler o nivel atual

E `Flex` encaixa exatamente nessa necessidade.

---

## 3. Decidir o delay concreto do driver

### O que o driver espera

O driver foi escrito com uma abstracao baseada em `DelayNs`.

Entao a integracao precisa passar um objeto concreto que consiga entregar esse delay curto.

### Observacao importante

Aqui existe um detalhe tecnico real do projeto:

- o plano ja dizia que o AM2302 exige temporizacao bloqueante em microssegundos
- `embassy_time::Timer` nao resolve isso
- usar apenas `delay_us(1)` dentro do loop de leitura nao foi suficiente na pratica
- a implementacao estavel precisou medir o pulso com `DWT::cycle_count()` e usar o `delay` apenas para a fase de start do protocolo

### O que foi adotado na implementacao que funcionou

O projeto manteve um `DelayNs` concreto pequeno em `main.rs` para:

- segurar a linha em low por `2 ms`
- esperar a janela curta de liberacao antes da resposta do sensor

Mas a leitura dos bits deixou de depender desse delay curto. Em vez disso, o driver passou a:

- usar `DWT::cycle_count()` para medir os pulsos high do frame
- usar um limiar em ciclos para separar `0` de `1`
- executar `begin_signal() + read_frame()` dentro de `cortex_m::interrupt::free(...)`

Exemplo conceitual do que fica em `main.rs`:

```rust
// Delay bloqueante curto usado apenas no start do protocolo.
let am2302_delay = Am2302Delay;
```

### O que importa neste passo

Neste passo 4, o importante e conseguir instanciar o driver com um delay real compativel com a placa e com a medicao fina baseada em ciclos habilitada no core.

### Habilitacao necessaria no boot

Para a implementacao baseada em `DWT` funcionar, a `main()` precisa habilitar o contador de ciclos logo apos `embassy_stm32::init(config)`:

```rust
let mut core = cortex_m::Peripherals::take().unwrap();
core.DCB.enable_trace();
core.DWT.enable_cycle_counter();
```

Sem isso, a leitura do frame volta a ficar cega para os pulsos curtos do AM2302.

---

## 4. Instanciar o driver `Am2302`

### Onde aplicar

Ainda em `src/main.rs`, dentro da `main()`.

### Como fica conceitualmente

```rust
use crate::drivers::am2302::Am2302;
```

E na inicializacao:

```rust
// Instancia o driver do AM2302 com a linha real e o delay concreto.
let am2302 = Am2302::new(am2302_line, am2302_delay);
```

### O que isso resolve

- conecta o driver ao hardware real
- deixa o sensor pronto para entrar em uma task periodica
- conecta o runtime a implementacao correta de timing fino usada pelo driver

---

## 5. Criar uma task periodica de leitura em `src/main.rs`

### Onde aplicar

Faca isso no proprio `src/main.rs`, junto das outras tasks `#[embassy_executor::task]` ja existentes.

### Estrutura sugerida

Uma forma simples seria:

```rust
use crate::drivers::am2302::Am2302;
use embassy_stm32::gpio::Flex;
```

```rust
#[embassy_executor::task]
async fn am2302_task(mut sensor: Am2302<Flex<'static>, YourDelayType>) {
    loop {
        match sensor.read() {
            Ok((temperature_c, humidity_rh)) => {
                // Loga as medicoes no RTT para validar o driver antes da shell.
                info!("AM2302: temp={} C, humidity={} %RH", temperature_c, humidity_rh);
            }

            Err(error) => {
                // Loga o erro para diagnosticar timeout, checksum ou protocolo.
                error!("AM2302 error: {:?}", error);
            }
        }

        // Respeita o intervalo minimo do datasheet entre leituras.
        Timer::after_secs(2).await;
    }
}
```

### Observacao importante

O tipo exato no parametro da task depende do delay concreto que voce criar.

Por isso, neste item o principal e a ideia estrutural:

- task dedicada
- `sensor.read()`
- `info!` em caso de sucesso
- `error!` em caso de falha
- espera de pelo menos `2 s`

---

## 6. Spawnar a task do AM2302 na `main()`

### Onde aplicar

No final da fase de runtime concorrente da `main()`.

### Como localizar o ponto exato

Procure onde hoje a `main()` faz `spawn(...)` de tarefas como:

- ADC
- botao
- shell
- PWM
- acelerometro
- LED

### Como fica

```rust
// Cria a task periodica do AM2302.
spawner.spawn(unwrap!(am2302_task(am2302)));
```

### O que isso resolve

- o sensor passa a ser lido automaticamente no runtime
- o teste deixa de depender de shell

---

## 7. Validar pelo terminal do PowerShell do VS Code

### Sim, este e o melhor teste neste passo

Como este passo ainda nao expoe `sensors` na shell, a validacao ideal e usar RTT no terminal com:

```powershell
cargo run
```

### O que deve acontecer

Depois da gravacao do firmware, o terminal deve mostrar os logs de boot e, se o sensor estiver funcionando, entradas periodicas como:

```text
AM2302: temp=24.7 C, humidity=58.3 %RH
```

### O que observar se der erro

Exemplos de falhas uteis para diagnostico:

```text
AM2302 error: Timeout
AM2302 error: Checksum
AM2302 error: Protocol
```

### Como interpretar

- `Timeout`
  - sensor nao respondeu ou a leitura perdeu sincronismo
- `Checksum`
  - sensor respondeu, mas os bits chegaram corrompidos
- `Protocol`
  - a sequencia medida nao bateu com o esperado

### Aprendizado real deste passo

Na bancada, o problema principal nao era o sensor nem o resistor de pull-up.

O problema era a estrategia antiga de timing:

- medir pulsos por contagem aproximada de loops com `delay_us(1)`
- deixar interrupcoes preemptarem a leitura do frame

Isso fazia o firmware perder pulsos de `26 us` e `70 us`, levando a `Timeout` e leitura de bits errados.

A implementacao estavel ficou assim:

- `Flex` em `PB6`
- `Am2302Delay` para o start do protocolo
- `DWT::cycle_count()` para medir os pulsos dos bits
- `interrupt::free(...)` envolvendo a leitura do frame

### Por que este teste e importante antes da shell

Porque ele isola as variaveis:

- hardware do sensor
- protocolo do driver
- temporizacao real do driver

sem ainda misturar com parser de comandos e interface UART.

---

## 8. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- comando `sensors` na shell
- armazenamento elaborado do ultimo valor lido
- filtros, medias, historico ou calibracao extra
- integracao com outros sensores no mesmo comando

Neste passo 4, o sucesso e apenas:

- o driver foi instanciado no `main.rs`
- a task do AM2302 foi spawnada
- os valores podem ser vistos pelo terminal RTT no PowerShell

---

## 9. Checklist de validacao

Antes de seguir para o passo 5, valide pelo menos isto:

1. `cargo check`
2. `cargo run`
3. a placa sobe normalmente
4. a task do AM2302 e spawnada
5. o terminal mostra leituras periodicas ou erros coerentes
6. o intervalo entre leituras e de pelo menos `2 s`

### Resultado esperado

Se este passo foi feito corretamente:

- o AM2302 ja esta integrado ao firmware real
- o comportamento pode ser acompanhado no terminal do VS Code
- o passo 5 pode focar apenas em expor as leituras na shell com `sensors`

---

## Resumo deste passo

O passo 4 da Fase 5 e a integracao do driver no runtime real do projeto.

As decisoes principais aqui sao:

- usar `PB6` / `PWM/CS/D10`
- criar a linha concreta com `Flex`
- instanciar `Am2302`
- habilitar `DWT` no boot
- medir pulsos do frame por ciclos de CPU
- proteger a leitura com secao critica
- criar uma task periodica dedicada
- validar pelo terminal do PowerShell do VS Code com `cargo run`

Entao, sim: **neste passo da para testar e confirmar o funcionamento antes de irmos para a shell**.

Na pratica, essa e justamente a forma mais segura de validar o sensor antes do comando `sensors`.
