clear# Fase 2 - Passo 4 da Shell UART

Este arquivo mostra como desenvolver o Passo 4 da Fase 2.

Escopo deste passo:

- garantir que a escrita da shell nao seja intrusiva
- manter `uart.write(...).await` como mecanismo de envio
- evitar respostas e logs excessivos
- preparar a shell para coexistir melhor com as outras tasks

Importante: neste passo, a ideia nao e mudar a arquitetura da shell, e sim refinar o comportamento de escrita para que o console fique usavel e o firmware nao gere ruido desnecessario.
---

## Arquivos que podem ser alterados

Neste passo, as alteracoes ficam concentradas em:

- `src/app/shell.rs`
- `src/main.rs`

---

## Objetivo pratico

Hoje a shell ja funciona, mas ainda existem dois pontos a melhorar:

1. a escrita deve continuar sempre assíncrona via `uart.write(...).await`
2. o excesso de mensagens de outras tasks pode atrapalhar o uso do console

Entao o Passo 4 faz dois refinamentos:

- manter a shell respondendo de forma enxuta
- reduzir mensagens repetitivas que nao sao essenciais para o uso normal

---

## 1. Manter a escrita da shell sempre assíncrona

### Onde verificar

No arquivo:

`src/app/shell.rs`

### O que garantir

Toda resposta da shell deve sair por chamadas deste tipo:

```rust
let _ = uart.write(response.as_bytes()).await;
```

ou:

```rust
let _ = uart.write(b"mensagem\r\n").await;
```

### Por que isso importa

- evita bloqueio manual prolongado
- deixa a UART integrada ao executor async
- mantem o comportamento coerente com o resto do firmware Embassy

### Neste passo

Se o seu `shell_task(...)` ja estiver usando `uart.write(...).await`, entao esta parte ja esta correta e nao precisa de nova mudanca estrutural.

---

## 2. Evitar respostas excessivamente longas na shell

### Onde ajustar

No arquivo:

`src/app/shell.rs`

### Como esta hoje

Seu `help` e `status` provavelmente ja sao curtos, o que e bom.

Exemplo conceitual:

```rust
"Comandos disponiveis:\r\n"
" - help: ...\r\n"
" - status: ...\r\n"
```

### O que manter como regra

- respostas curtas
- sem textos muito grandes
- sem despejar diagnostico demais por padrao

### Exemplo de estilo recomendado

```rust
"Sistema ativo\r\n"
"UART: ok\r\n"
"Shell: inicial\r\n"
```

### O que evitar

- respostas com muitos paragrafos
- comandos que imprimem muito sem necessidade
- repetir mensagem inicial da shell em loop

---

## 3. Reduzir prints excessivos das outras tasks

### Por que isso entra neste passo

Mesmo que a shell use UART e os logs usem RTT/defmt, muitas mensagens de background dificultam:

- observar o estado do firmware
- identificar erros reais
- usar o sistema de forma limpa durante os testes

### Onde alterar

No arquivo:

`src/main.rs`

### Principais pontos que podem estar gerando ruido

Exemplos tipicos do projeto atual:

- `adc_task()` imprimindo `ADC Valor` a cada 500 ms
- `accel_task()` imprimindo continuamente
- mensagens de erro do ADXL345 em toda tentativa falha

---

## 4. Ajuste sugerido para `adc_task()`

### Onde alterar

Em `src/main.rs`, dentro da task do ADC.

### Como pode estar hoje

```rust
loop {
    let measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);

    defmt::info!("ADC Valor: {}", measured);

    embassy_time::Timer::after_millis(500).await;
}
```

### Como pode ficar para reduzir ruido

```rust
loop {
    let measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);

    // Mantem a leitura ativa, mas evita flood de log durante o uso normal da shell.
    let _ = measured;

    embassy_time::Timer::after_millis(500).await;
}
```

### O que mudou

- a task continua funcionando
- o ADC continua sendo lido
- o log repetitivo deixa de poluir o terminal RTT

---

## 5. Ajuste sugerido para `accel_task()`

### Onde alterar

Em `src/main.rs`, dentro da task do acelerometro.

### Como pode estar hoje

```rust
loop {
    if let Ok((x, y, z)) = accel.get_accel().await {
        info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
    }
    Timer::after_millis(1000).await;
}
```

### Como pode ficar

```rust
loop {
    let _ = accel.get_accel().await;
    Timer::after_millis(1000).await;
}
```

### O que mudou

- o acesso ao sensor continua acontecendo
- o log continuo deixa de poluir a execucao

---

## 6. Ajuste sugerido para erros repetitivos do ADXL345

### Problema

Se o sensor estiver ausente ou com erro de ligacao, o firmware pode repetir erro demais.

### Onde isso costuma acontecer

No setup do acelerometro em `src/main.rs`.

### Regra recomendada

- manter o erro inicial
- evitar repetir a mesma mensagem sem necessidade durante testes da shell

### Exemplo de abordagem

Se o erro so acontece uma vez no boot, pode manter.
Se acontecer em loop, vale reduzir ou silenciar temporariamente durante a Fase 2.

---

## 7. O que alterar na shell neste passo

### Em `src/app/shell.rs`

Normalmente, neste passo voce nao precisa reescrever a estrutura da shell.

Voce deve apenas verificar que:

- `shell_task(...)` usa `uart.write(...).await`
- as respostas `help` e `status` continuam curtas
- erros de buffer e parse tem mensagens pequenas

Exemplos bons:

```rust
let _ = uart.write(b"Erro: linha muito longa.\r\n").await;
let _ = uart.write(b"Erro: argumentos demais.\r\n").await;
let _ = uart.write(b"Erro ao interpretar comando.\r\n").await;
```

---

## 8. O que alterar em `src/main.rs`

### Meta deste passo

Deixar o ambiente mais limpo para teste da shell.

### Ajustes sugeridos

1. remover ou silenciar logs repetitivos de `adc_task()`
2. remover ou silenciar logs repetitivos de `accel_task()`
3. manter apenas logs realmente importantes:
   - startup
   - erros de setup
   - eventos relevantes como botao, se quiser

---

## 9. Como estava e como deve ficar

### Exemplo 1: ADC

#### Antes

```rust
defmt::info!("ADC Valor: {}", measured);
```

#### Depois

```rust
let _ = measured;
```

---

### Exemplo 2: Acelerometro

#### Antes

```rust
info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
```

#### Depois

```rust
let _ = accel.get_accel().await;
```

ou, se quiser manter a leitura original:

```rust
if let Ok((_x, _y, _z)) = accel.get_accel().await {
    // leitura mantida sem log continuo
}
```

---

## 10. Como testar este passo

Depois das alteracoes:

1. rodar `cargo check`
2. rodar `cargo run`
3. verificar se o terminal RTT ficou mais limpo
4. abrir o terminal serial da shell
5. testar:

```text
help
status
comando_invalido
```

### Resultado esperado

- shell continua respondendo normalmente
- o terminal RTT fica com menos ruido
- fica mais facil validar o console serial

---

## 11. O que este passo conclui

Ao final deste passo, voce deve ter:

- escrita da shell mantida em modo async
- respostas curtas e objetivas
- menos ruido de logs concorrentes
- ambiente melhor para continuar a Fase 2 e iniciar testes das proximas fases

---

## 12. Resumo direto

Neste passo, voce deve:

1. manter `uart.write(...).await` na shell
2. evitar respostas muito grandes
3. reduzir logs repetitivos em `src/main.rs`
4. testar novamente o `help` e `status` em um terminal serial separado

O foco aqui nao e adicionar nova funcionalidade, e sim tornar o uso da shell mais limpo e menos intrusivo.
