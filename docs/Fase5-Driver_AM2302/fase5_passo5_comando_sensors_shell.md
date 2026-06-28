## Fase 5 - Passo 5: Expor o `AM2302` no comando `sensors`

Este arquivo descreve o quinto passo da Fase 5.

Objetivo deste passo:

- reaproveitar a leitura periodica do `AM2302` ja integrada no firmware
- armazenar o ultimo valor lido em um estado compartilhado
- criar o comando `sensors` na shell UART
- exibir temperatura, umidade e estado da ultima leitura sem reler o sensor dentro da shell

Neste momento, o foco ja nao e fazer o protocolo funcionar.

O protocolo do `AM2302` ja foi validado no hardware no passo 4.

Agora o foco e expor esse resultado de forma segura e simples para o usuario pela shell.

---

## O que o plano pede neste passo

No `PLANO_TRABALHO.md`, a Fase 5 define como quinto passo:

1. comando `sensors` no shell exibe valores

Entao este passo pede:

- integrar a leitura do `AM2302` com a shell
- exibir os dados mais recentes por comando textual

E ainda nao pede:

- incluir o `HC-SR04` no mesmo comando
- fazer historico de medicoes
- criar filtros, medias ou persistencia

Essas partes pertencem a fases e passos posteriores.

---

## Arquivos alterados neste passo

Neste passo, a implementacao deve se concentrar em:

- `src/main.rs`
- `src/app/shell.rs`

Tambem e natural criar ou ajustar um pequeno estado compartilhado para sensores em:

- `src/app/monitor.rs`

ou em algum modulo simples de app, se voce preferir separar monitoramento de tarefas e cache de sensores.

O importante e evitar espalhar a logica do `sensors` em muitos lugares.

---

## Resposta curta sobre a estrategia correta

### O comando `sensors` nao deve conversar direto com o AM2302

O comando da shell nao deve chamar `sensor.read()` diretamente.

Isso seria ruim porque:

- a leitura do `AM2302` e bloqueante e sensivel a timing fino
- o driver usa `DWT::cycle_count()` e secao critica durante o frame
- misturar isso com o caminho da shell deixaria a interface menos previsivel
- a shell poderia travar esperando uma nova leitura

Entao, neste passo, a estrategia correta e:

1. a task periodica do `AM2302` continua fazendo as leituras
2. o firmware guarda o ultimo resultado em memoria compartilhada
3. o comando `sensors` apenas le esse ultimo snapshot e monta a resposta textual

Essa abordagem respeita o que ja funcionou na bancada e mantem a shell leve.

---

## Visao geral da ideia

No fim do passo 4, o projeto ja tem:

- `PB6` ligado ao `AM2302`
- `Flex` como linha de dados
- `Am2302Delay` para o start do protocolo
- medicao dos pulsos com `DWT::cycle_count()`
- leitura protegida por `interrupt::free(...)`
- task periodica `am2302_task(...)`
- logs RTT com temperatura e umidade validas

Agora falta ligar isso ao comando textual da shell:

1. criar um estado compartilhado para a ultima leitura
2. atualizar esse estado dentro da task `am2302_task`
3. adicionar `sensors` na tabela de comandos
4. implementar o branch `"sensors" => { ... }` em `execute_command(...)`
5. responder com os valores mais recentes ou com o ultimo erro conhecido

---

## 1. Criar um estado compartilhado para o ultimo valor do AM2302

### O que precisa ser guardado

O comando `sensors` precisa ler um retrato simples do estado atual do sensor.

O minimo util e guardar:

- ultima temperatura em `C`
- ultima umidade em `%RH`
- timestamp da ultima atualizacao
- se a ultima leitura foi sucesso ou erro
- qual erro ocorreu, se houve falha

### Forma simples recomendada

Uma estrutura pequena e suficiente, por exemplo:

```rust
pub struct Am2302Snapshot {
    pub temperature_c: Option<f32>,
    pub humidity_rh: Option<f32>,
    pub last_update_ms: Option<u64>,
    pub last_error: Option<Am2302Error>,
}
```

### Onde guardar esse estado

A mesma estrategia ja usada pelo projeto para `MONITOR` funciona bem aqui:

```rust
pub(crate) static AM2302_STATE: Mutex<CriticalSectionRawMutex, Am2302Snapshot> =
    Mutex::new(Am2302Snapshot::new());
```

### Por que essa abordagem e boa

- combina com o restante do projeto
- evita reler o sensor pela shell
- permite responder imediatamente ao comando `sensors`
- facilita expor depois outros sensores no mesmo padrao

---

## 2. Atualizar esse estado dentro da task do AM2302

### Onde aplicar

Na propria `am2302_task(...)` em `src/main.rs`.

### O que deve acontecer em caso de sucesso

Quando `sensor.read()` retornar `Ok((temperature_c, humidity_rh))`, a task deve:

1. ler o tempo atual com `Instant::now().as_millis()`
2. abrir o estado compartilhado
3. atualizar temperatura, umidade e timestamp
4. limpar `last_error`
5. manter o `info!` de log RTT, se desejar

### O que deve acontecer em caso de erro

Quando `sensor.read()` retornar `Err(error)`, a task deve:

1. registrar o erro no snapshot compartilhado
2. preservar o ultimo valor valido, se isso for util para debug
3. continuar logando no RTT
4. esperar os `2 s` antes da proxima tentativa

### Decisao pratica importante

E melhor preservar o ultimo valor valido e registrar o ultimo erro separado.

Assim, o comando `sensors` pode mostrar algo como:

- ultima medicao valida
- idade dessa medicao
- ultimo erro observado depois dela

Isso ajuda no debug sem apagar informacao boa.

---

## 3. Adicionar o comando `sensors` na tabela da shell

### Onde aplicar

Em `src/app/shell.rs`, na lista `COMMANDS`.

### Como fica conceitualmente

Adicionar uma entrada como:

```rust
CommandEntry {
    name: "sensors",
    help: "Mostra a ultima leitura dos sensores",
},
```

### O que isso resolve

- faz `help` listar o novo comando
- mantem a tabela de comandos coerente com o plano

---

## 4. Implementar o branch `sensors` em `execute_command(...)`

### Onde aplicar

Tambem em `src/app/shell.rs`.

### Regra de uso recomendada

Neste passo, o comando pode nascer sem argumentos:

```text
sensors
```

Se o usuario mandar argumentos extras, a resposta deve seguir o padrao dos outros comandos:

```text
Erro: este comando nao aceita argumentos.
```

### Estrutura sugerida

Uma ideia simples:

1. verificar se `cmd.args` esta vazio
2. ler o snapshot compartilhado
3. montar a resposta textual em `ShellResponse`

### Exemplo conceitual de saida em sucesso

```text
Sensores:
AM2302 temp: 15.5 C
AM2302 humidity: 88.4 %RH
Atualizado ha: 1240 ms
```

### Exemplo conceitual quando ainda nao houve leitura valida

```text
Sensores:
AM2302: sem leitura valida ainda
```

### Exemplo conceitual quando ha erro recente

```text
Sensores:
AM2302 temp: 15.5 C
AM2302 humidity: 88.4 %RH
Atualizado ha: 1240 ms
Ultimo erro: Timeout
```

### Por que esse formato e bom

- e curto para UART
- mostra o que interessa sem poluir a interface
- separa valor atual de erro recente
- deixa espaco para incluir `HC-SR04` depois no mesmo comando

---

## 5. Decidir como integrar com o monitor atual

### Opcao mais simples

Expandir `src/app/monitor.rs` para tambem carregar o snapshot do `AM2302`.

Isso deixa tudo centralizado em uma unica estrutura global.

### Opcao mais limpa

Criar um estado separado so para sensores, por exemplo:

- `src/app/sensors_state.rs`

com uma `static` propria.

### Recomendacao para este projeto

Como o projeto ainda e pequeno, qualquer uma das duas funciona.

Mas, olhando a estrutura atual, a solucao mais pragmatica para este passo e:

- manter a mudanca pequena
- reaproveitar o padrao de `Mutex<CriticalSectionRawMutex, ...>` ja existente
- evitar criar muitos modulos novos sem necessidade

Entao, para este passo 5, a melhor escolha tende a ser expandir o estado compartilhado existente ou criar um estado simples no `main.rs` com a mesma abordagem.

---

## 6. O que nao fazer dentro do comando `sensors`

Para manter o comando robusto, evite:

- chamar `sensor.read()` dentro da shell
- bloquear a shell esperando o AM2302 responder
- reconstruir o driver dentro do comando
- misturar regra de protocolo com regra de interface UART

O comando `sensors` deve ser somente uma leitura de estado ja pronto.

---

## 7. Validar pelo terminal da shell

### Como testar

Depois de implementar o comando:

1. rode `cargo run`
2. espere a shell inicializar
3. digite `help`
4. confirme que `sensors` aparece na lista
5. digite `sensors`

### O que deve acontecer

O terminal UART deve responder com algo coerente com o que o RTT ja mostra.

Se o RTT estiver mostrando algo como:

```text
AM2302: temp=15.5 C, humidity=88.4 %RH
```

entao a shell deve conseguir mostrar esses mesmos dados em formato textual proprio.

### O que observar se der erro

Exemplos uteis de validacao:

- `help` lista `sensors`
- `sensors` responde sem travar a shell
- a resposta muda ao longo do tempo conforme novas leituras chegam
- quando ha erro no sensor, a shell mostra estado coerente

---

## 8. O que ainda nao fazer neste passo

Para manter este passo pequeno e seguro, ainda nao implemente:

- inclusao do `HC-SR04` no comando `sensors`
- subcomandos como `sensors raw`, `sensors force`, `sensors watch`
- leitura forcada do sensor por shell
- calibracao, medias, min/max ou historico persistente

Neste passo 5, o sucesso e apenas:

- existe um estado compartilhado com a ultima leitura do `AM2302`
- a task periodica atualiza esse estado
- o comando `sensors` existe na shell
- o comando responde com os valores mais recentes

---

## 9. Checklist de validacao

Antes de encerrar o passo 5, valide pelo menos isto:

1. `cargo check`
2. `cargo run`
3. a placa sobe normalmente
4. a shell continua inicializando normalmente
5. `help` lista `sensors`
6. `sensors` responde sem argumentos extras
7. a resposta mostra temperatura e umidade coerentes com o RTT
8. em caso de falha do sensor, a shell mostra estado coerente

### Resultado esperado

Se este passo foi feito corretamente:

- o `AM2302` passa a ser visivel pela shell
- a shell nao interfere no timing sensivel do driver
- o projeto fecha a Fase 5 com protocolo e exposicao textual funcionando

---

## Resumo deste passo

O passo 5 da Fase 5 e o momento em que a leitura do `AM2302` deixa de aparecer apenas no RTT e passa a ficar disponivel ao usuario pela shell UART.

As decisoes principais aqui sao:

- nao reler o sensor dentro do comando
- guardar a ultima leitura em estado compartilhado
- atualizar esse estado na task periodica do `AM2302`
- criar o comando `sensors` em `src/app/shell.rs`
- responder de forma curta, estavel e coerente com o estado atual do sensor

Na pratica, esse passo fecha a integracao do `AM2302` com a interface textual do firmware e prepara o terreno para incluir outros sensores no mesmo comando mais adiante.
