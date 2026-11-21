//! La nave del jugador: controles progresivos, gravedad y colision.
//!
//! Todo depende del tiempo transcurrido, nunca del numero de cuadros, asi
//! que la nave se comporta igual en una maquina rapida que en una lenta.
//! El acelerador, el freno y los tres giros se suavizan con un filtro
//! exponencial: los controles no son interruptores, tienen inercia.

use crate::math::{Mat4, Vec3};

/// Entrada del jugador en un cuadro. Cada valor va de -1 a 1.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShipInput {
    /// Adelante positivo, atras negativo.
    pub thrust: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Desplazamiento lateral y vertical sin girar.
    pub strafe: f32,
    pub lift: f32,
    pub boost: bool,
    pub brake: bool,
}

/// Modo de control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightMode {
    /// Vuelo libre por el sistema.
    Crucero,
    /// Vuelo pegado a un planeta: la nave se mantiene nivelada respecto de
    /// la superficie y es facil rodearlo sin perder el control.
    Local,
}

pub struct Ship {
    pub position: Vec3,
    /// Base ortonormal de la nave: derecha, arriba y adelante.
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub velocity: Vec3,

    /// Acelerador suavizado, de -1 a 1.
    pub throttle: f32,
    /// Velocidades angulares suavizadas, en radianes por segundo.
    pub rate_yaw: f32,
    pub rate_pitch: f32,
    pub rate_roll: f32,

    pub mode: FlightMode,
    /// Radio de colision de la nave.
    pub radius: f32,

    /// Empuje maximo en unidades por segundo al cuadrado.
    pub max_thrust: f32,
    pub boost_factor: f32,
    /// Rapidez con la que el acelerador alcanza su objetivo.
    pub throttle_response: f32,
    /// Rapidez con la que los giros alcanzan su objetivo.
    pub turn_response: f32,
    pub max_turn_rate: f32,
    /// Rozamiento artificial: sin el, la nave nunca se detendria.
    pub drag: f32,
}

impl Default for Ship {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 6.0, 46.0),
            right: Vec3::new(1.0, 0.0, 0.0),
            up: Vec3::UP,
            forward: Vec3::new(0.0, 0.0, -1.0),
            velocity: Vec3::ZERO,
            throttle: 0.0,
            rate_yaw: 0.0,
            rate_pitch: 0.0,
            rate_roll: 0.0,
            mode: FlightMode::Crucero,
            radius: 0.6,
            max_thrust: 26.0,
            boost_factor: 3.4,
            throttle_response: 2.6,
            turn_response: 5.0,
            max_turn_rate: 1.5,
            drag: 0.55,
        }
    }
}

/// Factor de un filtro exponencial independiente de la tasa de cuadros.
///
/// `1 - exp(-k*dt)` se acerca a `k*dt` cuando dt es chico, pero nunca pasa
/// de 1, asi que tampoco se desestabiliza con cuadros largos.
#[inline]
fn suavizado(k: f32, dt: f32) -> f32 {
    1.0 - (-k * dt).exp()
}

impl Ship {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_basis(self.right, self.up, self.forward, self.position)
    }

    /// Reconstruye la base para que no se deforme por acumulacion de
    /// error al girar miles de veces.
    fn ortonormalizar(&mut self) {
        self.forward = self.forward.normalized();
        self.right = (self.up.cross(self.forward)).normalized();
        if self.right == Vec3::ZERO {
            self.right = Vec3::new(1.0, 0.0, 0.0);
        }
        self.up = self.forward.cross(self.right).normalized();
    }

    /// Aplica los controles y avanza la fisica `dt` segundos.
    ///
    /// `up_referencia` es la vertical local cuando se vuela cerca de un
    /// planeta; en crucero se pasa `None`.
    pub fn update(&mut self, input: &ShipInput, dt: f32, up_referencia: Option<Vec3>) {
        let dt = dt.clamp(0.0, 0.1);

        // Giros progresivos: la velocidad angular persigue a la entrada.
        let k = suavizado(self.turn_response, dt);
        self.rate_yaw += (input.yaw * self.max_turn_rate - self.rate_yaw) * k;
        self.rate_pitch += (input.pitch * self.max_turn_rate - self.rate_pitch) * k;
        self.rate_roll += (input.roll * self.max_turn_rate * 1.4 - self.rate_roll) * k;

        let giro = Mat4::rotation_axis(self.up, -self.rate_yaw * dt)
            * Mat4::rotation_axis(self.right, self.rate_pitch * dt)
            * Mat4::rotation_axis(self.forward, -self.rate_roll * dt);
        self.forward = giro.mul_dir(self.forward);
        self.up = giro.mul_dir(self.up);
        self.right = giro.mul_dir(self.right);
        self.ortonormalizar();

        // En modo local la nave se nivela sola respecto de la superficie.
        if self.mode == FlightMode::Local {
            if let Some(vertical) = up_referencia {
                let k = suavizado(2.2, dt);
                // Se quita de "arriba" la parte que apunta al frente para
                // no torcer la direccion de vuelo.
                let objetivo = (vertical - self.forward * vertical.dot(self.forward)).normalized();
                if objetivo != Vec3::ZERO {
                    self.up = (self.up + (objetivo - self.up) * k).normalized();
                    self.ortonormalizar();
                }
            }
        }

        // Acelerador progresivo.
        let k = suavizado(self.throttle_response, dt);
        self.throttle += (input.thrust - self.throttle) * k;

        let empuje = self.max_thrust * if input.boost { self.boost_factor } else { 1.0 };
        self.velocity += self.forward * (self.throttle * empuje * dt);
        self.velocity += self.right * (input.strafe * empuje * 0.35 * dt);
        self.velocity += self.up * (input.lift * empuje * 0.35 * dt);

        // Freno: reduce la velocidad de forma suave hasta casi detenerla.
        if input.brake {
            let f = (-3.5 * dt).exp();
            self.velocity = self.velocity * f;
            self.throttle *= f;
        }

        // Rozamiento constante, tambien independiente de los cuadros.
        self.velocity = self.velocity * (-self.drag * dt).exp();
        self.position += self.velocity * dt;
    }

    /// Suma la atraccion de un cuerpo durante `dt` segundos.
    ///
    /// Es la ley del inverso del cuadrado, con un radio minimo para que la
    /// fuerza no se dispare al acercarse al centro.
    pub fn apply_gravity(&mut self, center: Vec3, mass: f32, dt: f32, g: f32) {
        let d = center - self.position;
        let dist2 = d.length_squared().max(1.0);
        let dist = dist2.sqrt();
        let a = g * mass / dist2;
        self.velocity += d * (a * dt / dist);
    }

    /// Impide atravesar un cuerpo.
    ///
    /// Si la nave entra en la esfera de colision se la vuelve a colocar
    /// justo afuera y se anula la componente de velocidad que apuntaba
    /// hacia adentro, de modo que resbala por la superficie en vez de
    /// quedarse pegada.
    pub fn resolve_collision(&mut self, center: Vec3, surface_radius: f32) -> bool {
        let d = self.position - center;
        let dist = d.length();
        let minimo = surface_radius + self.radius;
        if dist >= minimo || dist < 1e-5 {
            return false;
        }
        let n = d / dist;
        self.position = center + n * minimo;
        let hacia_adentro = self.velocity.dot(n);
        if hacia_adentro < 0.0 {
            // Se quita la componente normal y se pierde algo de energia.
            self.velocity = (self.velocity - n * hacia_adentro) * 0.82;
        }
        true
    }
}
