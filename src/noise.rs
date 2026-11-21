//! Ruido procedural con semilla.
//!
//! Todo el aspecto del sistema sale de aqui: la deformacion de la
//! geometria de los planetas, los patrones de los shaders, la distorsion
//! animada del sol y las posiciones de los objetos de superficie.
//!
//! La misma semilla produce siempre el mismo resultado, en cualquier
//! maquina y en cualquier corrida.

use crate::math::Vec3;

/// Generador congruencial de 32 bits. Sirve para sembrar posiciones
/// reproducibles sin depender de ninguna libreria externa.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Se evita el estado cero, que se quedaria pegado.
        Self { state: seed | 1 }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        // xorshift32
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Flotante uniforme en [0, 1).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Punto uniforme sobre la esfera unitaria, por el metodo de la banda
    /// de Arquimedes: z uniforme en [-1, 1] y angulo uniforme.
    pub fn unit_sphere(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, crate::math::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * a.cos(), r * a.sin(), z)
    }
}

/// Campo de ruido tridimensional.
#[derive(Debug, Clone, Copy)]
pub struct Noise {
    seed: u32,
}

#[inline]
fn fade(t: f32) -> f32 {
    // Quintica de Perlin: primera y segunda derivada nulas en los bordes,
    // lo que evita las bandas visibles del interpolado lineal.
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

impl Noise {
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    pub const fn seed(&self) -> u32 {
        self.seed
    }

    /// Valor pseudoaleatorio en [-1, 1] asociado a una celda entera.
    #[inline]
    fn hash(&self, x: i32, y: i32, z: i32) -> f32 {
        let mut h = (x as u32)
            .wrapping_mul(0x9E3779B1)
            ^ (y as u32).wrapping_mul(0x85EBCA77)
            ^ (z as u32).wrapping_mul(0xC2B2AE3D)
            ^ self.seed.wrapping_mul(0x27D4EB2F);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2545F491);
        h ^= h >> 13;
        h = h.wrapping_mul(0x3B9ACB93);
        h ^= h >> 16;
        (h as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Ruido de valor trilineal con interpolacion quintica. Devuelve
    /// aproximadamente [-1, 1].
    pub fn value(&self, p: Vec3) -> f32 {
        let xi = p.x.floor();
        let yi = p.y.floor();
        let zi = p.z.floor();
        let (x0, y0, z0) = (xi as i32, yi as i32, zi as i32);
        let (fx, fy, fz) = (fade(p.x - xi), fade(p.y - yi), fade(p.z - zi));

        let c = |dx: i32, dy: i32, dz: i32| self.hash(x0 + dx, y0 + dy, z0 + dz);

        let x00 = c(0, 0, 0) + (c(1, 0, 0) - c(0, 0, 0)) * fx;
        let x10 = c(0, 1, 0) + (c(1, 1, 0) - c(0, 1, 0)) * fx;
        let x01 = c(0, 0, 1) + (c(1, 0, 1) - c(0, 0, 1)) * fx;
        let x11 = c(0, 1, 1) + (c(1, 1, 1) - c(0, 1, 1)) * fx;

        let y0v = x00 + (x10 - x00) * fy;
        let y1v = x01 + (x11 - x01) * fy;
        y0v + (y1v - y0v) * fz
    }

    /// Suma de octavas (ruido fractal). Cada octava duplica la frecuencia
    /// y reduce la amplitud, y el resultado se normaliza a [-1, 1].
    pub fn fbm(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut norm = 0.0;
        let mut q = p;
        for _ in 0..octaves {
            sum += self.value(q) * amp;
            norm += amp;
            amp *= gain;
            q = q * lacunarity;
            // Un pequeño corrimiento por octava rompe la rejilla que se
            // forma cuando todas comparten el mismo origen.
            q = q + Vec3::new(37.1, 17.7, 91.3);
        }
        if norm <= 0.0 {
            0.0
        } else {
            sum / norm
        }
    }

    /// Variante con crestas: produce cordilleras y grietas en vez de
    /// colinas suaves.
    pub fn ridged(&self, p: Vec3, octaves: u32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 0.5;
        let mut norm = 0.0;
        let mut q = p;
        for _ in 0..octaves {
            let v = 1.0 - self.value(q).abs();
            sum += v * v * amp;
            norm += amp;
            amp *= 0.5;
            q = q * 2.0 + Vec3::new(11.3, 53.7, 29.1);
        }
        if norm <= 0.0 {
            0.0
        } else {
            sum / norm
        }
    }

    /// Ruido celular: distancia al punto sembrado mas cercano. Da
    /// crateres, placas de hielo y escamas.
    pub fn cellular(&self, p: Vec3) -> f32 {
        let base = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
        let frac = p - base;
        let mut best = f32::INFINITY;

        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cell = Vec3::new(dx as f32, dy as f32, dz as f32);
                    let ix = base.x as i32 + dx;
                    let iy = base.y as i32 + dy;
                    let iz = base.z as i32 + dz;
                    // Punto dentro de la celda, estable para esa celda.
                    let jitter = Vec3::new(
                        self.hash(ix, iy, iz) * 0.5 + 0.5,
                        self.hash(ix + 101, iy - 73, iz + 17) * 0.5 + 0.5,
                        self.hash(ix - 57, iy + 131, iz - 29) * 0.5 + 0.5,
                    );
                    let d = (cell + jitter - frac).length_squared();
                    if d < best {
                        best = d;
                    }
                }
            }
        }
        best.sqrt().min(1.0)
    }

    /// Distorsion del dominio: desplaza el punto de muestreo con otro
    /// ruido antes de evaluar. Es el efecto de "warping" que hace que la
    /// superficie del sol parezca fluir.
    pub fn warp(&self, p: Vec3, strength: f32) -> Vec3 {
        let q = Vec3::new(
            self.fbm(p + Vec3::new(0.0, 0.0, 0.0), 4, 2.0, 0.5),
            self.fbm(p + Vec3::new(5.2, 1.3, 9.7), 4, 2.0, 0.5),
            self.fbm(p + Vec3::new(2.8, 7.1, 3.4), 4, 2.0, 0.5),
        );
        p + q * strength
    }
}
