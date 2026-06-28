
#![no_std]
#![no_main]
mod app;
mod drivers;
use crate::app::led_task::{led_task, LedControl};
use crate::app::shell::shell_taks;
use crate::app::monitor::SystemMonitor;
use crate::drivers::am2302::Am2302;

//use cortex_m::Peripherals;
use core::arch::asm;
use cortex_m_rt::pre_init;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::adc::{Adc, AdcChannel, AnyAdcChannel, SampleTime};
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, OutputType, Pull, Speed};
use embassy_stm32::i2c::{self, I2c};
use adxl345_async::{Address, Adxl345Async, DataRate, I2cBus, Range};
use embassy_stm32::peripherals::{self, ADC2};
use embassy_stm32::time::{khz, Hertz};
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::usart::{self, Uart};
use embassy_stm32::gpio::Flex;
use embassy_stm32::{Config, Peri, bind_interrupts, interrupt};
//use embassy_time::Timer;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Instant, Timer};
use futures::future::Select;
use {defmt_rtt as _, panic_probe as _};

//use embassy_stm32::timer::pwm_input::PwmInput;
//use embassy_stm32::time::hz;
//use embassy_stm32::timer::CountingMode;
//use embassy_stm32::rcc::low_level::RccPeripheral;
//use embassy_stm32::timer::low_level::GeneralPurpose16bitInstance;
//use embassy_stm32::pac::metadata::Peripheral;

type AsyncI2c =
    I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>;
type AccelDevice = Adxl345Async<I2cBus<AsyncI2c>>;

const ADDRESS: u8 = 0x53;
const WHOAMI: u8 = 0;

//Essas declarações apenas dizem ao compilador Rust que esses símbolos existem no
//  binário final e serão resolvidos pelo linker
unsafe extern "C"
{
    //Inicio e fim da seção .text no binário
    static __stext: u8;
    static __etext: u8;

    //Inicio e fim da seção .data em RAM
    static __sdata: u8;
    static __edata: u8;

    //Inicio e fim da seção .bss em RAM
    static __sbss: u8;
    static __ebss: u8;
}

// Permite que outros módulos do projeto leaim o monitor compartilhado 
pub(crate) static MONITOR: Mutex<CriticalSectionRawMutex, SystemMonitor> =
    Mutex::new(SystemMonitor::new());

pub(crate) static LED_CONTROL: Mutex<CriticalSectionRawMutex, LedControl> = 
    Mutex::new(LedControl::new());

//bind_interrupts!(struct Irqs {
//    TIM2 => timer::CaptureCompareInterruptHandler<peripherals::TIM2>;
//});

bind_interrupts!(struct Irqs {
    LPUART1 => embassy_stm32::usart::InterruptHandler<peripherals::LPUART1>;
    DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH2>;

    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<peripherals::I2C1>;
    DMA1_CHANNEL3 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH3>;
    DMA1_CHANNEL4 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH4>;
    EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

//#[link_section = ".ram2bss"]
#[link_section = ".ccmram"]
static TESTE: i32 = 60;

#[link_section = ".data2"]
static TESTE2: i32 = 70;

#[pre_init]
unsafe fn before_main() {
    unsafe {
        asm! {
            "ldr r0, =__sccmdata
            ldr r1, =__eccmdata
            ldr r2, =__siccmdata
            0:
            cmp r1, r0
            beq 1f
            ldm r2!, {{r3}}
            stm r0!, {{r3}}
            b 0b
            1:"
        }

        asm! {
            "ldr r0, =__sdata2
            ldr r1, =__edata2
            ldr r2, =__sidata2
            2:
            cmp r1, r0
            beq 3f
            ldm r2!, {{r3}}
            stm r0!, {{r3}}
            b 2b
            3:"
        }
    }
}

/*
#[interrupt]
unsafe fn TIM3(){

    // reset interrupt flag
    //unsafe {
    //    let pin = embassy_stm32::peripherals::PA5::steal();
    //    let mut pin = Output::new(pin, Level::High, Speed::Low);
    //    pin.set_high();
    //}
    //pac::TIM3.sr().modify(|r| r.set_uif(false));
    info!("interrupt happens: tim20");
}
     */

struct Am2302Delay;

impl Am2302Delay
{
    fn new() -> Self
    {
        Self
    }
}

impl embedded_hal::delay::DelayNs for Am2302Delay 
{
    fn delay_ns(&mut self, ns: u32) 
    {
        let cycles = (170_000_000u64 * ns as u64) / 1_000_000_000u64;
        cortex_m::asm::delay(cycles as u32);
    }

    fn delay_us(&mut self, us: u32) 
    {
        let cycles = 170 * us;
        cortex_m::asm::delay(cycles);
    }

    fn delay_ms(&mut self, ms: u32) 
    {
        for _ in 0..ms {
            self.delay_us(1000);
        }
    }
}

fn section_size(start: *const u8, end: *const u8) -> usize 
{
    //Calcula o tamanho Bruto da seção em bytes 
    (end as usize).saturating_sub(start as usize)
}

pub(crate) fn firmware_memory_sizes() -> (usize, usize, usize) {
    let text_size = section_size(core::ptr::addr_of!(__stext), core::ptr::addr_of!(__etext));
    let data_size = section_size(core::ptr::addr_of!(__sdata), core::ptr::addr_of!(__edata));
    let bss_size = section_size(core::ptr::addr_of!(__sbss), core::ptr::addr_of!(__ebss));

    (text_size, data_size, bss_size)
}

//Retorna a configuração de clocks do STM32G474
// HSE externo de 24 MHz, PLL configurado para 170 MHz no sistema.
// Os prescalers AHB/APB1/APB2 ficam em DIV1 (sem divisao).
fn stm32_config() -> Config {
    use embassy_stm32::rcc::*;

    // Usa o Config do embassy_stm32 inteiro;
    let mut config = embassy_stm32::Config::default();

    // HSE: oscilador externo de 24 MHz (cristal da placa).
    config.rcc.hse = Some(Hse {
        freq: Hertz(24_000_000),
        mode: HseMode::Oscillator,
    });

    // PLL: entrada 24 MHz / 6 = 4 MHz, * 85 = 340 MHz, / 2 = 170 MHz.
    config.rcc.pll = Some(Pll {
        source: PllSource::HSE,
        prediv: PllPreDiv::DIV6,
        mul: PllMul::MUL85,
        divp: None,
        divq: None,
        //Clock principal em 170 MHz, como no firmware atual
        divr: Some(PllRDiv::DIV2),
    });

    // Clock do sistema vindo do PLL1_R.
    config.rcc.sys = Sysclk::PLL1_R;

    //ADC12 usa o clock do sistema diretamente
    config.rcc.mux.adc12sel = mux::Adcsel::SYS;

    //Nenhum prescaler nos barramentos principais 
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    config
}

//Loga mensagens iniciais e verifica as variáveis de memória 
//Se os valores aparecerem corretos no log, as seções especiais 
// foram copiadas com sucesso
fn log_startup() {
    info!("Hello World!");

    //TESTE esta em .ccmram, TESTE 2 esta em .data2(SRAM2)
    println!("Teste de variavel na memória CCRAM {}", TESTE);
    println!("Teste de variavel na memória SRAM2 {}", TESTE2);
}

//Configura o botao do usuário (PC13) com interrupção EXRI
//Usa Pull::Down interno para que o estado padrão 
//  seja LOW e a borda de subida indique o pressionamento
fn init_button(
    pc13: Peri<'static, peripherals::PC13>,
    exti13: Peri<'static, peripherals::EXTI13>,
) -> ExtiInput<'static, embassy_stm32::mode::Async> {
    //Irqs contem o binding de interrupção EXTI para esse canal.
    ExtiInput::new(pc13, exti13, Pull::Down, Irqs)
}

//Cria o ADC2 e o canal analogico a partir do pino PA7
//Retorna uma tupla (Adc, AnyAdcChannel) para ser usada tanto
//  no self-test quanto na task adc_task
fn init_adc(
    adc2: Peri<'static, peripherals::ADC2>,
    pa7: Peri<'static, peripherals::PA7>,
) -> (Adc<'static, ADC2>, AnyAdcChannel<'static, ADC2>) {
    //Adc: new com Default>>default() usa configuração padrão do Embassy 
    let adc = Adc::new(adc2, Default::default());

    //degrade_adc() converte o pino Pa7 em um canal analogico 
    let adc_channel = pa7.degrade_adc();

    (adc,adc_channel)
}

//Faz uma leitura unica do ADC para validar o hardware ja no boot
//O valor lido e logado via defmt. Se aparecer variando, o ADC
//  está funcionando. Se ficar em zero ou máximo, algo pode estar errado
fn log_initial_adc_sample(
    adc: &mut Adc<'static, ADC2>,
    adc_channel: &mut AnyAdcChannel<'static, ADC2>,
) {
    //blocking_read é síncrono, trava a task atual 
    let measured = adc.blocking_read(adc_channel, SampleTime::CYCLES247_5);
    info!("measured: {}", measured);
}

//Le o sensor de temperatura inteiro do chip uma vez no startup
//Loga o valor cru. Não faz conversão para graus Celsius,
// apenas confirma que o sensor responde 
fn log_temperature_sample(adc1: Peri<'static, peripherals::ADC1>) {
    //ADC1 é usado exclusivamente para o sensor interno 
    let mut adc_temp = Adc::new(adc1, Default::default());

    //Habilita o sensor de temperatura interno do chip 
    let mut temperature = adc_temp.enable_temperature();

    //Leitura única, síncrona 
    let temp = adc_temp.blocking_read(&mut  temperature, SampleTime::CYCLES247_5);
    info!("measured: {}", temp);
}

// Configura a LPUART assíncrona com DMA
// Bautrate de 115200, pinos PA3 (RX) e PA2 (TX).
//Usa DMA1 CH1 e CH2 para RX e TX
fn init_uart(
    lpuart1: Peri<'static, peripherals::LPUART1>,
    pa3: Peri<'static, peripherals::PA3>,
    pa2: Peri<'static, peripherals::PA2>,
    dma1_ch1: Peri<'static, peripherals::DMA1_CH1>,
    dma1_ch2: Peri<'static, peripherals::DMA1_CH2>,
) -> Uart<'static, embassy_stm32::mode::Async> {
    let mut config = usart::Config::default();
    config.baudrate = 115_200;

    //Uart: new com DMA. Irqs contem os handlers de interrupção 
    Uart::new(lpuart1, pa3, pa2, dma1_ch1, dma1_ch2, Irqs, config).unwrap()
}

//Conmfigura o TIM1 como PWM simples no canal 1 (PC0)
//Frequência de 10 kHz, push-pull, duty cycle iniciado em 0
fn init_pwm(
    tim1: Peri<'static, peripherals::TIM1>,
    pc0: Peri<'static, peripherals::PC0>,
) -> SimplePwm<'static, peripherals::TIM1> {
    let ch1_pin = PwmPin::new(pc0, OutputType::PushPull);

    //SimplePwm com somente o canal 1 ativo
    SimplePwm::new(
        tim1, 
        Some(ch1_pin),
        None, 
        None, 
        None, 
        khz(10), 
    Default::default(),
    )
}

//Inicializa o barramento I2C1, detecta o ADXL245, configura o 
//  driver e faz uma leitura inicial de validação 
//Retorna o driver Adxl345Async pronto para a task accel_task
async fn init_accelerometer(
    i2c1: Peri<'static, peripherals::I2C1>,
    pb8: Peri<'static, peripherals::PB8>,
    pb9: Peri<'static, peripherals::PB9>,
    dma1_ch3: Peri<'static, peripherals::DMA1_CH3>,
    dma1_ch4: Peri<'static, peripherals::DMA1_CH4>,
    pc9: Peri<'static, peripherals::PC9>,
) -> AccelDevice {
    // Configuracao do barramento I2C.
    let mut config: i2c::Config = Default::default();
    config.scl_pullup = true;
    config.sda_pullup = true;
    config.frequency = Hertz(100_000);
    config.timeout = embassy_time::Duration::from_millis(1000);

    // I2C1 com DMA (CH3 e CH4).
    let mut i2c = I2c::new(i2c1, pb8, pb9, dma1_ch3, dma1_ch4, Irqs, config);

    // A linha PC9 e usada como controle de habilitacao.
    let mut i2c_cs = Output::new(pc9, Level::Low, Speed::Low);
    Timer::after_millis(50).await;

    let mut data = [0u8; 1];
    i2c_cs.set_high();
    Timer::after_millis(5).await;

    match i2c.write_read(ADDRESS, &[WHOAMI], &mut data).await {
        Ok(()) => {
            if data[0] == 0xE5 {
                info!("ADXL345 found!");
            } else {
                info!("Whoami: {}", data[0]);
            }
        }
        Err(e) => error!("I2c Error: {:?}", e),
    }

    let bus = I2cBus::new(i2c, Some(Address::SECONDARY));
    let mut accel = Adxl345Async::new(bus);

    let _ = accel.setup().await;

    match accel.get_accel_raw().await {
        Ok((x, y, z)) => {
            info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
        }
        Err(e) => error!("Error reading accel: {:?}", e),
    }

    accel
}

// Declare async tasks
#[embassy_executor::task]
async fn adc_task(mut adc: Adc<'static, ADC2>, mut adc_pin: AnyAdcChannel<'static, ADC2>) {
    loop {
        // Na versão 0.6, o SampleTime mudou e é passado direto no método de leitura
        // Mantem a aquisicao ativa sem poluir o RTT durante o uso da shell.
        let _measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);

        //Registra que a task ADC executou neste ciclo 
        mark_adc_execution().await;

        // Evita travar a CPU em busy-waiting infinito na task
        embassy_time::Timer::after_millis(500).await;
    }
}

// Declare async tasks
#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static, embassy_stm32::mode::Async>) {
    info!("Press the USER button...");

    loop {
        button.wait_for_rising_edge().await;

        //Conta um novo evento de press no monitoramento
        mark_button_execution().await;
        info!("Pressed!");

        button.wait_for_falling_edge().await;
        info!("Released!");
    }
}

// Declare async tasks
#[embassy_executor::task]
async fn uart_task(mut lpuart: Uart<'static, embassy_stm32::mode::Async>) {
    info!("UART started, type something...");
    lpuart
        .write("UART started, type something...".as_bytes())
        .await
        .unwrap();

    let mut buffer = [0u8; 1];

    // Loop to read from UART and echo back
    loop {
        lpuart.read(&mut buffer).await.unwrap();
        lpuart.write(&buffer).await.unwrap();
    }
}

// Declare async tasks
#[embassy_executor::task]
async fn pwm_task(mut pwm: SimplePwm<'static, embassy_stm32::peripherals::TIM1>) {
    let mut ch1 = pwm.ch1();
    ch1.enable();

    // Loop to read from UART and echo back
    loop {
        ch1.set_duty_cycle_fully_off();
        Timer::after_millis(300).await;
        ch1.set_duty_cycle_fraction(1, 4);
        Timer::after_millis(300).await;
        ch1.set_duty_cycle_fraction(1, 2);
        Timer::after_millis(300).await;
        ch1.set_duty_cycle(ch1.max_duty_cycle() - 1);
        Timer::after_millis(300).await;
    }
}

// Declare async tasks
#[embassy_executor::task]
async fn accel_task(mut accel: AccelDevice) {
    let _ = accel.set_range(Range::G2).await;
    let _ = accel.set_data_rate(DataRate::Rate100Hz).await;
    loop {
        if let Ok((x, y, z)) = accel.get_accel().await {
            info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
        }
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::task]
async fn am2302_task(mut sensor: Am2302<Flex<'static>, Am2302Delay>) {
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

async fn mark_adc_execution()
{
    //Captura o instante atual em milissegundos 
    let now_ms = Instant::now().as_millis() as u64;

    //Abre o monitor compartilhado por um período curto
    let mut monitor = MONITOR.lock().await;

    //Atualiza somente a métrica da task ADC
    monitor.adc.mark_execution(now_ms); 
}

async fn mark_button_execution()
{
    //Captura o instante atual em milissegundos 
    let now_ms = Instant::now().as_millis() as u64;

    //Abre o monitor compartilhado por um período curto
    let mut monitor = MONITOR.lock().await;

    //Atualiza somente a métrica da task do botão
    monitor.button.mark_execution(now_ms); 
}

pub(crate) async fn mark_led_execution()
{
        //Captura o instante atual em milissegundos 
    let now_ms = Instant::now().as_millis() as u64;

    //Abre o monitor compartilhado por um período curto
    let mut monitor = MONITOR.lock().await;

    //Atualiza somente a métrica da task ADC
    monitor.led.mark_execution(now_ms); 
}

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    // === 1. Configuração do chip ===
    //Cria a configuração do clock e inicializa o microcontrolador 
    let  config = stm32_config();
    let p = embassy_stm32::init(config);

    // Habilita o contador de ciclos do core para medir os pulsos do AM2302.
    let mut core = cortex_m::Peripherals::take().unwrap();
    core.DCB.enable_trace();
    core.DWT.enable_cycle_counter();

    let am2302_line = Flex::new(p.PB6);
    let am2302_delay = Am2302Delay;
    let am2302 = Am2302::new(am2302_line, am2302_delay);

    // === 2. Logs de startup ===
    //Exibe o Hello World e verifica as seções especiais de memória 
    log_startup();

    // === 3. Self-checks de hardware 
    //Leitura unica do ADC em PA7 e do sensor de temperatura interno 
    //Serve para validar que o ADC responde já no boot 
    let (mut adc, mut adc_channel) = init_adc(p.ADC2, p.PA7);
    log_initial_adc_sample(&mut adc, &mut adc_channel);
    log_temperature_sample(p.ADC1);

    // === 4. Inicializando os periféricos ===
    //Cada periférico ganha sua própria função de setup 
    let button = init_button(p.PC13, p.EXTI13);
    let lpuart1 = init_uart(
        p.LPUART1, 
        p.PA3, 
        p.PA2, 
        p.DMA1_CH1, 
        p.DMA1_CH2
    );
    let pwm = init_pwm(p.TIM1, p.PC0);
    let accel = init_accelerometer(
        p.I2C1, 
        p.PB8, 
        p.PB9, 
        p.DMA1_CH3, 
        p.DMA1_CH4, 
        p.PC9
    ).await;

    // === 5. Runtime concorrente ===
    //Cada periférico 
    spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
    spawner.spawn(unwrap!(button_task(button)));
    //spawner.spawn(unwrap!(uart_task(lpuart1)));
    spawner.spawn(unwrap!(shell_taks(lpuart1)));
    spawner.spawn(unwrap!(pwm_task(pwm)));
    spawner.spawn(unwrap!(accel_task(accel)));
    spawner.spawn(unwrap!(led_task(p.PA5)));
    spawner.spawn(unwrap!(am2302_task(am2302)));

    // === 6. Loop Principal ===
    // Mantem a main viva enquanto as tasks rodam em paralelo.
    loop {
        Timer::after_secs(1).await;
    }
}
