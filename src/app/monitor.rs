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
    pub max_interval_ms: Option<u64>
}

impl TaskMetrics
{
    pub const fn new(name: &'static str) -> Self{
        Self
        {
            name,
            execution_count: 0,
            last_run_ms: None,
            min_interval_ms: None,
            max_interval_ms: None,
        }
    }

    pub fn mark_execution(&mut self, now_ms: u64)
    {
        //Se já havia uma execução anterior, calcula o intervalo a partir dela 
        if let  Some(previous_run_ms) = self.last_run_ms{
            //saturating_sub: evita underflow caso now_ms seja menor que previous_run_ms
            let interval_ms = now_ms.saturating_sub(previous_run_ms);

            //Na primeira medição de intervalo, inicializa o mínimo.
            //Depois disso, mantém sempre o menor valor observado
            self.min_interval_ms = match self.min_interval_ms {
                Some(current_min_ms) => Some(current_min_ms.min(interval_ms)),
                None => Some(interval_ms),
            };

            //Na primeira medição de intervalo, inicializa o máximo 
            //Depois disso, matém sempre o maior valor observado 
            self.max_interval_ms = match self.max_interval_ms {
                Some(current_max_ms) => Some(current_max_ms.max(interval_ms)),
                None => Some(interval_ms),
            };

        }

        //saturating_add evita overflow em execuções muito longas 
        self.execution_count = self.execution_count.saturating_add(1);

        //Registra o timestamp da execução mais recente 
        self.last_run_ms = Some(now_ms);
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
        }
    }
}