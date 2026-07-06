use crate::drivers::am2302_capture::Am2302CaptureError;
use crate::drivers::Hc_Sr_04::HcSr04Error;
use crate::drivers::hcsr04::PulseMeasureError;
//Debug permite inspecionar a struct em logs de depuração 
// Clone + Copy permitem copiar a struct por valor sem complexidade extra 
#[derive(Debug, Clone, Copy)]
pub struct TaskMetrics
{
    //Nome fixo da task, por exemplo: "adc" ou "button"
    //&'static str: aponta para uma string literal que vive o programa inteiro 
    pub name: &'static str,

    //Quantas vezes a task marcou execucao 
    pub execution_count: u32,

    //Timestamp da última execução em milisegundos 
    // Option é usada porque, no inicio, a task ainda pode não ter executdo 
    pub last_run_ms: Option<u64>,

    //Menor intervalo observado entre duas execucoes consecutivas 
    // Também começa como None porque ainda não existe intervalo antes da segunda execução 
    pub min_interval_ms : Option<u64>,

    //Maior intervalo observado entre duas execuções consecutivas 
    pub max_interval_ms: Option<u64>,

    //Intevalo nominal esperado entre executores, em milissegundos 
    pub expected_interval_ms: Option<u64>,

    //Ultimo intervalo observado entre duas execuções consecutivas
    pub last_interval_ms: Option<u64>,

    //Soma acumulada dos valores absolutos de jitter 
    pub jitter_accum: u64,

    //Quantas amostras de jitter acumuladas até agora 
    pub jitter_samples: u32,

}

impl TaskMetrics
{
    pub const fn new(name: &'static str) -> Self
    {
        Self
        {
            name,
            execution_count: 0,
            last_run_ms: None,
            min_interval_ms: None,
            max_interval_ms: None,

            expected_interval_ms: None,
            last_interval_ms: None,
            jitter_accum: 0,
            jitter_samples: 0,
        }
    }

    pub fn set_expected_interval(&mut self, interval_ms: u64)
    {
        self.expected_interval_ms = Some(interval_ms);
    }

    pub fn mark_execution(&mut self, now_ms: u64)
    {
        if let Some(previous_run_ms) = self.last_run_ms
        {
            let interval_ms = now_ms.saturating_sub(previous_run_ms);

            self.last_interval_ms = Some(interval_ms);

            self.min_interval_ms = match self.min_interval_ms
            {
                Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
                None => Some(interval_ms),
            };

            self.max_interval_ms = match self.max_interval_ms
            {
                Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
                None => Some(interval_ms),
            };

            if let Some(expected_ms) = self.expected_interval_ms
            {
                let jitter = if interval_ms > expected_ms
                {
                    interval_ms - expected_ms
                } else {
                    expected_ms - interval_ms
                };

                self.jitter_accum = self.jitter_accum.saturating_add(jitter);
                self.jitter_samples = self.jitter_samples.saturating_add(1);
            }
        }

        self.execution_count = self.execution_count.saturating_add(1);
        self.last_run_ms = Some(now_ms);
    }

    pub fn average_jitter_ms(&self) -> Option<u64>
    {
        if self.jitter_samples > 0
        {
            Some(self.jitter_accum / self.jitter_samples as u64)
        }
        else
        {
            None
        }
    }
}

pub struct SystemMonitor
{
    //Métricas da task ADC
    pub adc: TaskMetrics,

    //Métricas da taks do botão 
    pub button: TaskMetrics,

    // Métricas da task de led
    pub led: TaskMetrics,

    //Métricas da task do sensor AM2302
    pub am2302_task: TaskMetrics,

    //Métricas da task do sensor HC-SR04
    pub hcsr04_task: TaskMetrics,

    //Snapshot mais recente do AM2302
    pub am2302: Am2302Snapshot,

    //Snapshot mais recente do HC-SR04
    pub hcsr04: HcSr04Snapshot,

}

impl SystemMonitor
{
    pub const fn new() -> Self
    {
        Self
        {
            //Inicializa a entrada do ADC com nome fixo 
            adc: TaskMetrics::new("adc"),

            //Inicializa a entrada do botão com nome fixo 
            button: TaskMetrics::new("button"),

            //Inicializa a entrada do LED com nome fixo 
            led: TaskMetrics::new("led"),

            //Inicializa a entrada do AM2302 com nome fixo
            am2302_task: TaskMetrics::new("am2302"),

            //Inicializa a entrada do HC-SR04 com nome fixo
            hcsr04_task: TaskMetrics::new("hcsr04"),

            //Estado inicial do AM2302 antes da primeira leitura 
            am2302: Am2302Snapshot::new(),

            hcsr04: HcSr04Snapshot::new(),
        }
    }
}

//Snapshot simples com o último estado conhecido do sensor 
#[derive(Debug, Clone, Copy)]
pub struct Am2302Snapshot
{
    pub temperature_c: Option<f32>,
    pub humidity_rh: Option<f32>,
    pub last_update_ms: Option<u64>,
    pub last_error: Option<Am2302CaptureError>,
}

impl Am2302Snapshot 
{
    //Estado inicial: ainda não existe leitura válida nem erro conhecido 
    pub const  fn new() -> Self
    {
        Self
        {
            temperature_c: None,
            humidity_rh: None,
            last_update_ms: None,
            last_error: None,
        }
    } 

    //Atualiza o snapshot quando uma leitura termina com sucesso 
    pub fn update_success(&mut self, temperature_c: f32, humidity_rh: f32, now_ms: u64)
    {
        self.temperature_c = Some(temperature_c);
        self.humidity_rh = Some(humidity_rh);
        self.last_update_ms = Some(now_ms);
        self.last_error = None;
    }   

    //Registra o snapshot quando uma leitura termina com sucesso 
    pub fn update_error(&mut self, error: Am2302CaptureError)
    {
        self.last_error = Some(error);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HcSr04Snapshot
{
    pub distance_cm: Option<f32>,
    pub last_update_ms: Option<u64>,
    pub last_error: Option<HcSr04Error<PulseMeasureError>>,
}

impl  HcSr04Snapshot 
{
    //Estado inicial
    pub const  fn new() -> Self
    {
        Self
        {
            distance_cm: None,
            last_update_ms: None,
            last_error: None,
        }
    } 

    //Atualiza o snapshot quando uma medição termina com sucesso 
    pub fn update_success(&mut self, distance_cm: f32, now_ms: u64)
    {
        self.distance_cm = Some(distance_cm);
        self.last_update_ms = Some(now_ms);
        self.last_error = None;
    }   

    pub fn update_error(&mut self, error: HcSr04Error<PulseMeasureError>)
    {
        self.last_error = Some(error);
    }
}