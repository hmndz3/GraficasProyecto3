//! Recorrido guionado para la grabacion del video.
//!
//! Describe el vuelo completo como una lista de tramos. Cada tramo fija
//! alrededor de que cuerpo gira la nave, a que distancia empieza y
//! termina, cuantas vueltas da y a que altura sobre el plano. Entre dos
//! tramos consecutivos las posiciones se mezclan, de modo que el viaje de
//! un planeta a otro sale solo y sin cortes.
//!
//! La nave nunca se teletransporta: su posicion es una funcion continua
//! del tiempo y su orientacion se saca de la direccion en la que se mueve.

use crate::math::{smoothstep, Vec3, TAU};
use crate::scene::camera::Camera;
use crate::scene::ship::Ship;
use crate::scene::system::SolarSystem;

/// Un tramo del recorrido.
#[derive(Debug, Clone, Copy)]
pub struct Tramo {
    /// Indice del cuerpo alrededor del cual gira la nave.
    pub cuerpo: usize,
    /// Segundo en que empieza el tramo.
    pub inicio: f32,
    pub duracion: f32,
    /// Distancia al centro al principio y al final, en radios del cuerpo.
    pub radio_inicial: f32,
    pub radio_final: f32,
    /// Vueltas completas que da la nave durante el tramo.
    pub vueltas: f32,
    /// Desfase del punto de llegada respecto de la direccion al sol, en
    /// radianes. Con cero, la nave termina el tramo justo entre la
    /// estrella y el cuerpo, es decir con la cara iluminada de frente.
    pub fase: f32,
    /// Altura sobre el plano de la orbita, en radios del cuerpo.
    pub altura: f32,
    /// Distancia de la camara a la nave, en radios del cuerpo.
    pub camara: f32,
    /// Hasta que distancia -en radios del cuerpo- la camara sigue
    /// encuadrando al cuerpo en vez de mirar hacia adelante.
    pub encuadre: f32,
}

/// Datos que produce un tramo en un instante dado.
#[derive(Debug, Clone, Copy)]
struct Muestra {
    posicion: Vec3,
    centro: Vec3,
    distancia_camara: f32,
    /// Cuanto tira el cuerpo del punto de mira, de 0 a 1.
    peso_encuadre: f32,
}

pub struct Tour {
    pub tramos: Vec<Tramo>,
    /// Segundos que tarda la mezcla entre dos tramos.
    pub mezcla: f32,
}

impl Tour {
    /// Recorrido por omision: abre con el sistema completo, visita los
    /// cinco planetas y dos lunas, y termina pegado a la estrella.
    pub fn completo() -> Self {
        // (cuerpo, duracion, radio inicial, radio final, vueltas, fase,
        //  altura, camara, encuadre)
        let guion: [(usize, f32, f32, f32, f32, f32, f32, f32, f32); 9] = [
            // Vista general: la nave cruza el sistema por encima del
            // plano ecliptico, con la estrella siempre encuadrada.
            (0, 11.0, 13.0, 7.5, 0.30, 1.1, 2.2, 2.4, 26.0),
            // TERRANOVA: acercamiento y vuelta completa.
            (2, 12.0, 8.0, 2.25, 1.15, 0.55, 0.50, 2.5, 11.0),
            // SELENE, su luna.
            (3, 8.0, 8.0, 3.4, 1.0, 0.70, 0.5, 3.2, 12.0),
            // AURORA y sus anillos.
            (4, 13.0, 7.0, 2.3, 1.0, 0.65, 0.85, 2.4, 10.0),
            // IO MENOR.
            (5, 7.0, 8.0, 3.2, 0.9, 0.75, 0.4, 3.2, 12.0),
            // GELIDA, bosque de cristal.
            (6, 11.0, 8.0, 1.95, 1.15, 0.50, 0.45, 2.6, 11.0),
            // VULCANO, cordilleras y torres.
            (7, 11.0, 8.0, 1.85, 1.15, 0.60, 0.40, 2.6, 11.0),
            // FERRUM, crateres.
            (1, 9.0, 8.0, 2.6, 1.0, 0.55, 0.45, 3.0, 12.0),
            // Regreso a la estrella.
            (0, 11.0, 7.5, 2.4, 0.55, 2.0, 0.55, 2.2, 14.0),
        ];

        let mut tramos = Vec::new();
        let mut t = 0.0;
        for (cuerpo, duracion, r0, r1, vueltas, fase, altura, camara, encuadre) in guion {
            tramos.push(Tramo {
                cuerpo,
                inicio: t,
                duracion,
                radio_inicial: r0,
                radio_final: r1,
                vueltas,
                fase,
                altura,
                camara,
                encuadre,
            });
            t += duracion;
        }

        Self { tramos, mezcla: 2.2 }
    }

    pub fn duracion(&self) -> f32 {
        self.tramos
            .last()
            .map(|t| t.inicio + t.duracion)
            .unwrap_or(0.0)
    }

    /// Indice del tramo activo y cuanto se mezcla con el siguiente.
    fn tramo_en(&self, t: f32) -> (usize, f32) {
        let mut i = 0;
        for (k, tr) in self.tramos.iter().enumerate() {
            if t >= tr.inicio {
                i = k;
            }
        }
        let tr = &self.tramos[i];
        let restante = tr.inicio + tr.duracion - t;
        let k = if i + 1 < self.tramos.len() && restante < self.mezcla {
            smoothstep(self.mezcla, 0.0, restante.max(0.0))
        } else {
            0.0
        };
        (i, k)
    }

    /// Lo que dicta un tramo concreto en el instante `t`, aunque ese
    /// instante caiga fuera de su intervalo. Eso es lo que permite
    /// mezclar dos tramos sin saltos.
    fn muestra(&self, sistema: &SolarSystem, indice: usize, t: f32) -> Muestra {
        let tr = &self.tramos[indice];
        let cuerpo = &sistema.bodies[tr.cuerpo];
        let radio_base = cuerpo.outer_radius();

        let avance = ((t - tr.inicio) / tr.duracion).clamp(-0.6, 1.6);

        // Perfil de distancia: la nave se acerca durante los primeros dos
        // tercios del tramo, se queda cerca un momento y luego se separa
        // un poco. El acercamiento tiene que terminar antes de que empiece
        // la mezcla con el tramo siguiente, o el punto mas cercano nunca
        // llegaria a verse.
        let acerca = smoothstep(0.0, 0.62, avance);
        let aleja = smoothstep(0.80, 1.18, avance) * 0.45;
        let suave = (acerca - aleja).clamp(0.0, 1.0);
        let distancia =
            radio_base * (tr.radio_inicial + (tr.radio_final - tr.radio_inicial) * suave);

        // El giro se mide desde la direccion en la que esta la estrella,
        // y se ajusta para que el final del tramo -el punto mas cercano-
        // caiga sobre la cara iluminada. Si el angulo fuera absoluto, la
        // nave llegaria al lado nocturno de la mitad de los planetas.
        let hacia_sol = sistema.bodies[0].position - cuerpo.position;
        let referencia = if hacia_sol.length() > 1e-3 {
            hacia_sol.z.atan2(hacia_sol.x)
        } else {
            0.0
        };
        let angulo = referencia + tr.fase + (avance - 1.0) * TAU * tr.vueltas;
        let (s, c) = angulo.sin_cos();
        let offset = Vec3::new(c * distancia, tr.altura * radio_base, s * distancia);
        let posicion = cuerpo.position + offset;

        // El encuadre se suelta cuando la nave se aleja del cuerpo: asi,
        // durante un viaje largo la camara mira hacia adelante en lugar
        // de quedarse apuntando al vacio.
        let d = (posicion - cuerpo.position).length();
        let limite = radio_base * tr.encuadre;
        let peso = 1.0 - smoothstep(limite * 0.45, limite, d);

        Muestra {
            posicion,
            centro: cuerpo.position,
            // La camara nunca se pega demasiado a la nave, aunque el
            // cuerpo visitado sea pequeño.
            distancia_camara: (tr.camara * radio_base).max(4.2),
            peso_encuadre: peso,
        }
    }

    /// Posicion de la nave en el segundo `t` del recorrido.
    pub fn posicion(&self, sistema: &SolarSystem, t: f32) -> Vec3 {
        let (i, k) = self.tramo_en(t);
        let a = self.muestra(sistema, i, t);
        if k <= 0.0 {
            return a.posicion;
        }
        let b = self.muestra(sistema, i + 1, t);
        a.posicion.lerp(b.posicion, k)
    }

    /// Coloca la nave y la camara para el instante `t`.
    ///
    /// La direccion de vuelo se obtiene por diferencias finitas sobre la
    /// propia trayectoria, asi que la nave siempre apunta hacia donde va.
    pub fn aplicar(&self, sistema: &SolarSystem, t: f32, nave: &mut Ship, camara: &mut Camera) {
        let h = 0.08;
        let (i, k) = self.tramo_en(t);
        let a = self.muestra(sistema, i, t);
        let b = if k > 0.0 {
            self.muestra(sistema, i + 1, t)
        } else {
            a
        };

        let p = a.posicion.lerp(b.posicion, k);
        let dist_cam = a.distancia_camara + (b.distancia_camara - a.distancia_camara) * k;

        let p_antes = self.posicion(sistema, (t - h).max(0.0));
        let p_despues = self.posicion(sistema, t + h);

        let mut adelante = (p_despues - p_antes).normalized();
        if adelante == Vec3::ZERO {
            adelante = (a.centro - p).normalized();
        }

        nave.position = p;
        nave.forward = adelante;
        // La vertical de la nave se inclina hacia afuera del cuerpo que
        // esta rodeando: queda peraltada, como un avion en viraje.
        let radial = (p - a.centro).normalized();
        let referencia = (Vec3::UP * 0.6 + radial * 0.4).normalized();
        nave.right = referencia.cross(adelante).normalized();
        if nave.right == Vec3::ZERO {
            nave.right = Vec3::new(1.0, 0.0, 0.0);
        }
        nave.up = adelante.cross(nave.right).normalized();
        // Velocidad aparente, solo para que la llama del motor reaccione.
        nave.velocity = adelante * ((p_despues - p_antes).length() / (2.0 * h));
        nave.throttle = (nave.velocity.length() / 45.0).clamp(0.2, 1.0);

        // El punto de mira es hacia donde va la nave, atraido hacia el
        // cuerpo que se esta visitando. Cada tramo calcula su propio
        // objetivo y despues se mezclan los dos: mezclar directamente los
        // centros de dos cuerpos lejanos dejaria la camara mirando al
        // vacio durante los viajes.
        let adelante_lejos = p + adelante * (dist_cam * 1.8);
        let mira_a = adelante_lejos.lerp(a.centro, 0.8 * a.peso_encuadre);
        let mira_b = adelante_lejos.lerp(b.centro, 0.8 * b.peso_encuadre);
        let mira = mira_a.lerp(mira_b, k);

        // La camara se coloca detras de la nave sobre la linea que va de
        // la nave al punto de mira. Si se colocara detras segun la
        // direccion de vuelo, al encuadrar un planeta que queda de lado
        // la nave se saldria del cuadro.
        let mut dir_mira = (mira - p).normalized();
        if dir_mira == Vec3::ZERO {
            dir_mira = adelante;
        }
        camara.position =
            p - dir_mira * dist_cam + nave.up * (dist_cam * 0.20) + nave.right * (dist_cam * 0.26);
        camara.target = mira;
        camara.up = Vec3::UP;
    }
}
