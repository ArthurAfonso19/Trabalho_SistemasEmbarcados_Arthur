#![no_std]
#![no_main]

//use cortex_m::Peripherals;
use core::arch::asm;
use cortex_m_rt::pre_init;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::adc::{Adc, AdcChannel, AnyAdcChannel, SampleTime};
use embassy_stm32::gpio::{Level, Output, OutputType, Pull, Speed};
use embassy_stm32::i2c::{self, I2c};
use embassy_stm32::peripherals::ADC2;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{self, Uart};
use embassy_stm32::Config;
use embassy_stm32::{bind_interrupts, interrupt, peripherals};
//use adxl345_eh_driver::{Driver, address, GRange, OutputDataRate};
use adxl345_async::{Address, Adxl345Async, DataRate, I2cBus, Range};
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::time::khz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

//use embassy_stm32::timer::pwm_input::PwmInput;
//use embassy_stm32::time::hz;
//use embassy_stm32::timer::CountingMode;
//use embassy_stm32::rcc::low_level::RccPeripheral;
//use embassy_stm32::timer::low_level::GeneralPurpose16bitInstance;
//use embassy_stm32::pac::metadata::Peripheral;

// Declare async tasks
#[embassy_executor::task]
async fn adc_task(mut adc: Adc<'static, ADC2>, mut adc_pin: AnyAdcChannel<'static, ADC2>) {
    loop {
        // Na versão 0.6, o SampleTime mudou e é passado direto no método de leitura
        let measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);

        defmt::info!("ADC Valor: {}", measured);

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
async fn accel_task(
    mut accel: Adxl345Async<
        I2cBus<I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>>,
    >,
) {
    let _ = accel.set_range(Range::G2).await;
    let _ = accel.set_data_rate(DataRate::Rate100Hz).await;
    loop {
        if let Ok((x, y, z)) = accel.get_accel().await {
            info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
        }
        Timer::after_millis(1000).await;
    }
}

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

const ADDRESS: u8 = 0x53;
const WHOAMI: u8 = 0;

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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    //unsafe {
    //TESTE = 10;
    //TESTE2 = 20;
    //}
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(24_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV6,
            mul: PllMul::MUL85,
            divp: None,
            divq: None,
            // Main system clock at 170 MHz
            divr: Some(PllRDiv::DIV2),
        });
        config.rcc.sys = Sysclk::PLL1_R;
        //config.rcc.mux = ClockSrc::PLL;

        //config.rcc.adc12_clock_source = AdcClockSource::SYS;
        config.rcc.mux.adc12sel = mux::Adcsel::SYS;
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV1;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }

    let mut p: embassy_stm32::Peripherals = embassy_stm32::init(config);

    info!("Hello World!");
    //unsafe {
    println!("Teste de variável na memória CCMRAM {}", TESTE);
    println!("Teste de variável na memória SRAM2 {}", TESTE2);
    //}
    //defmt::println!("Hello, world!");

    //let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);

    let mut adc = Adc::new(p.ADC2, Default::default());

    let mut adc_temp = Adc::new(p.ADC1, Default::default());
    let mut temperature = adc_temp.enable_temperature();

    // Antes: p.PA7.reborrow_adc()
    let measured = adc.blocking_read(&mut p.PA7, SampleTime::CYCLES247_5); // Note a mudança no ciclo também!
    info!("measured: {}", measured);

    // Antes: temperature.reborrow_adc()
    // (Certifique-se de que o objeto do sensor de temperatura interna do chip se chama p.ADC_TEMP ou similar no G4)
    let temp = adc_temp.blocking_read(&mut temperature, SampleTime::CYCLES247_5);
    info!("measured: {}", temp);

    // 1. Cria o canal genérico a partir do pino PA7 configurando o SampleTime
    let adc_channel = p.PA7.degrade_adc();

    // Spawned tasks run in the background, concurrently.
    spawner.spawn(unwrap!(adc_task(adc, adc_channel)));

    spawner.spawn(unwrap!(button_task(button)));

    let mut config = usart::Config::default();
    config.baudrate = 115_200;
    let lpusart = Uart::new(
        p.LPUART1, p.PA3, p.PA2, p.DMA1_CH1, p.DMA1_CH2, Irqs, config,
    )
    .unwrap();
    spawner.spawn(unwrap!(uart_task(lpusart)));

    let ch1_pin = PwmPin::new(p.PC0, OutputType::PushPull);
    let pwm: SimplePwm<'_, embassy_stm32::peripherals::TIM1> = SimplePwm::new(
        p.TIM1,
        Some(ch1_pin),
        None,
        None,
        None,
        khz(10),
        Default::default(),
    );
    //let mut ch1: embassy_stm32::timer::simple_pwm::SimplePwmChannel<'_, embassy_stm32::peripherals::TIM1> = pwm.ch1();
    //ch1.enable();

    spawner.spawn(unwrap!(pwm_task(pwm)));

    //let mut i2c = I2c::new_blocking(p.I2C1, p.PB8, p.PB9, Hertz(100_000), i2c::Config::default());
    //println!("{:?}", p.PB8.af_num());

    let mut config: i2c::Config = Default::default();
    config.scl_pullup = true;
    config.sda_pullup = true;
    config.frequency = Hertz(100_000); // 100kHz I2C speed
    config.timeout = embassy_time::Duration::from_millis(1000);

    let mut i2c = I2c::new(p.I2C1, p.PB8, p.PB9, p.DMA1_CH3, p.DMA1_CH4, Irqs, config);

    //info!("AF: {:?}", p.PB8.af_num());
    //info!("AF: {:?}", p.PB9.af_num());

    let mut i2c_cs = Output::new(p.PC9, Level::Low, Speed::Low);
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
    //i2c_cs.set_low();

    let bus = I2cBus::new(i2c, Some(Address::SECONDARY));
    let mut accel = Adxl345Async::new(bus);
    let _ = accel.setup().await;
    match accel.get_accel_raw().await {
        Ok((x, y, z)) => {
            info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
        }
        Err(e) => error!("Error reading accel: {:?}", e),
    }

    spawner.spawn(unwrap!(accel_task(accel)));

    let mut led = Output::new(p.PA5, Level::High, Speed::Low);

    loop {
        //info!("high");
        led.set_high();
        Timer::after_millis(500).await;

        //info!("low");
        led.set_low();
        Timer::after_millis(500).await;
    }
}
