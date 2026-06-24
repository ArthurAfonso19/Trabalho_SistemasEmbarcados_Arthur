# Melhoria da Reorganizacao do `main.rs`

Esta reorganizacao melhorou a estrutura do firmware sem alterar o comportamento que ja estava funcionando na placa.

## O que melhorou

- A `main()` deixou de concentrar toda a inicializacao e passou a atuar como orquestradora.
- Cada periferico ganhou uma funcao de setup com responsabilidade mais clara.
- Os testes de startup ficaram separados das tasks de execucao continua.
- O codigo ficou mais facil de ler, manter e depurar.

## Separacao entre setup e runtime

Uma das principais melhorias foi separar:

- setup do sistema e dos perifericos
- validacoes de startup
- runtime assincrono das tasks

Exemplos:

- `init_adc()` cria o ADC e o canal
- `log_initial_adc_sample()` faz apenas a leitura inicial de validacao
- `adc_task()` continua executando a leitura periodica em background

Isso deixa mais claro o que acontece no boot e o que continua rodando durante a aplicacao.

## Uso correto de funcoes sincronas e assincronas

Outra melhoria foi manter como `fn` as rotinas que nao precisam de `.await`, por exemplo:

- `stm32_config()`
- `init_button()`
- `init_adc()`
- `init_uart()`
- `init_pwm()`

E manter como `async fn` apenas o que realmente depende de espera assincrona, por exemplo:

- `init_accelerometer()`
- `run_led_loop()`
- as tasks como `uart_task()`, `button_task()` e `accel_task()`

Vantagem disso:

- menos complexidade desnecessaria
- melhor leitura da intencao de cada funcao
- menor acoplamento com o executor assincrono

## Beneficios praticos

- Fica mais facil localizar problemas por periferico.
- Fica mais simples alterar uma parte sem mexer em todo o `main.rs`.
- A estrutura prepara melhor o projeto para novas etapas.
- O comportamento validado no hardware foi preservado.

## Resumo

Essa nova implementacao nao mudou o objetivo do firmware, mas melhorou sua organizacao.
Ela traz mais clareza, separa melhor responsabilidades e reduz a dificuldade de manutencao e expansao do projeto.
