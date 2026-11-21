//! Construccion y dibujo del sistema solar.
//!
//! Aqui se arma la escena completa -estrella, planetas, lunas, anillos,
//! objetos de superficie y fondo- y se dibuja en el orden correcto: fondo,
//! cuerpos opacos, anillos y capas aditivas al final.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::geometry::mesh::Mesh;
use crate::geometry::models;
use crate::geometry::sphere::{icosphere, ring};
use crate::geometry::terrain::{displace_sphere, Terrain, TerrainKind};
use crate::math::{to_radians, Mat4, Vec3, TAU};
use crate::noise::Noise;
use crate::render::framebuffer::Framebuffer;
use crate::render::frustum::Frustum;
use crate::render::pipeline::{DrawOptions, RasterStats, Uniforms};
use crate::render::raster::draw_mesh;
use crate::render::texture::generate_normal_map;
use crate::scene::body::{Atmosphere, Body, Orbit, PropBatch, PropInstance, RingSystem};
use crate::scene::camera::Camera;
use crate::scene::props::{scatter, PropRecipe};
use crate::scene::ship::Ship;
use crate::scene::skybox::Skybox;
use crate::shaders::star::CoronaShader;
use crate::shaders::{
    AtmosphereShader, GasShader, GlowShader, IceShader, OceanShader, PropShader, RingShader,
    RockyShader, ShipShader, StarShader, TintShader,
};

/// Niveles de subdivision de los planetas, del mas detallado al mas
/// sencillo.
const LODS_PLANETA: [u32; 4] = [4, 3, 2, 1];
const LODS_LUNA: [u32; 3] = [3, 2, 1];

/// Constante de gravitacion del juego. No busca ser realista: esta
/// ajustada para que la atraccion se sienta sin volver imposible el vuelo.
pub const G: f32 = 2.2;

/// Interruptores que permiten demostrar en vivo que el descarte y los
/// niveles de detalle cambian el trabajo del renderizador.
#[derive(Debug, Clone, Copy)]
pub struct RenderToggles {
    pub frustum_culling: bool,
    pub lod: bool,
    pub props: bool,
    pub orbitas: bool,
    pub atmosferas: bool,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            frustum_culling: true,
            lod: true,
            props: true,
            orbitas: true,
            atmosferas: true,
        }
    }
}

pub struct SolarSystem {
    pub bodies: Vec<Body>,
    pub skybox: Skybox,

    /// Shader de cada uno de los cuatro modelos de superficie.
    pub prop_shaders: Vec<PropShader>,

    pub ship_mesh: Mesh,
    pub ship_shader: ShipShader,
    pub flare_mesh: Mesh,
    pub flare_shader: GlowShader,

    /// Esfera unitaria reutilizada para las capas de atmosfera.
    pub atmosphere_mesh: Mesh,
    pub orbit_meshes: Vec<(usize, Mesh)>,
    pub orbit_shader: TintShader,

    pub corona_mesh: Mesh,
    pub corona_shader: CoronaShader,

    /// Segundos simulados desde el arranque.
    pub time: f32,
}

impl SolarSystem {
    /// Arma el sistema completo. La semilla fija todo lo procedural: el
    /// relieve, las nebulosas, las estrellas y la siembra de objetos.
    pub fn new(seed: u32) -> Self {
        let mut bodies: Vec<Body> = Vec::new();

        // ---------------------------------------------------------------
        // 0. HELIOS: la estrella.
        // ---------------------------------------------------------------
        let sol_radio = 6.2;
        let sol_terreno = Terrain::flat(seed);
        bodies.push(Body {
            name: "HELIOS".to_string(),
            radius: sol_radio,
            orbit: Orbit { radius: 0.0, speed: 0.0, phase: 0.0, inclination: 0.0 },
            parent: None,
            spin_speed: 0.045,
            axial_tilt: 0.0,
            terrain: sol_terreno,
            lods: construir_lods(sol_radio, &sol_terreno, &LODS_LUNA),
            shader: Box::new(StarShader::new(seed ^ 0x51a1)),
            normal_map: None,
            atmosphere: None,
            rings: None,
            props: Vec::new(),
            prop_batches: Vec::new(),
            mass: 2600.0,
            emissive: true,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // ---------------------------------------------------------------
        // 1. FERRUM: mundo de crateres, sin atmosfera.
        // ---------------------------------------------------------------
        let terreno = Terrain::new(TerrainKind::Craters, 0.055, 3.4, seed ^ 0x11);
        let nmap = Arc::new(generate_normal_map(&terreno, 512, 256, 1.6));
        bodies.push(Body {
            name: "FERRUM".to_string(),
            radius: 1.15,
            orbit: Orbit { radius: 22.0, speed: 0.155, phase: 0.6, inclination: 0.0 },
            parent: Some(0),
            spin_speed: 0.38,
            axial_tilt: to_radians(6.0),
            terrain: terreno,
            lods: construir_lods(1.15, &terreno, &LODS_PLANETA),
            shader: Box::new(RockyShader {
                noise: Noise::new(seed ^ 0x1a1),
                normal_map: nmap.clone(),
                normal_strength: 1.5,
                paleta: vec![
                    (0.0, Vec3::new(0.11, 0.08, 0.07)),
                    (0.42, Vec3::new(0.33, 0.21, 0.15)),
                    (0.70, Vec3::new(0.52, 0.36, 0.25)),
                    (1.0, Vec3::new(0.74, 0.60, 0.46)),
                ],
                detalle: 16.0,
                especular: 0.05,
            }),
            normal_map: Some(nmap),
            atmosphere: None,
            rings: None,
            props: Vec::new(),
            prop_batches: Vec::new(),
            mass: 24.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // ---------------------------------------------------------------
        // 2. TERRANOVA: continentes, oceano, nubes y vegetacion.
        // ---------------------------------------------------------------
        let terreno = Terrain::new(TerrainKind::Continents, 0.042, 2.1, seed ^ 0x22);
        let nmap = Arc::new(generate_normal_map(&terreno, 512, 256, 1.1));
        let radio = 1.75;
        let recipe = PropRecipe {
            seed: seed ^ 0xA10E,
            attempts: 9000,
            max_instances: 260,
            min_separation: 0.065,
            cluster_scale: 2.6,
            cluster_threshold: 0.08,
            min_height: 0.03,
            weights: [5.0, 1.6, 0.5, 0.7],
            scale_range: (0.65, 1.5),
            tints: [
                Vec3::new(0.62, 1.05, 0.55),
                Vec3::new(0.95, 0.88, 0.80),
                Vec3::new(0.75, 0.95, 1.15),
                Vec3::new(1.00, 0.95, 0.88),
            ],
        };
        let props = scatter(&recipe, &terreno, radio);
        bodies.push(Body {
            name: "TERRANOVA".to_string(),
            radius: radio,
            orbit: Orbit { radius: 34.0, speed: 0.098, phase: 2.4, inclination: 0.0 },
            parent: Some(0),
            spin_speed: 0.26,
            axial_tilt: to_radians(18.0),
            terrain: terreno,
            lods: construir_lods(radio, &terreno, &LODS_PLANETA),
            shader: Box::new(OceanShader {
                noise: Noise::new(seed ^ 0x2b2),
                normal_map: nmap.clone(),
                normal_strength: 1.1,
                nivel_mar: 0.02,
                agua_profunda: Vec3::new(0.008, 0.035, 0.125),
                agua_baja: Vec3::new(0.050, 0.340, 0.460),
                paleta_tierra: vec![
                    (0.0, Vec3::new(0.62, 0.56, 0.34)),
                    (0.18, Vec3::new(0.22, 0.40, 0.17)),
                    (0.55, Vec3::new(0.30, 0.34, 0.20)),
                    (0.80, Vec3::new(0.45, 0.42, 0.38)),
                    (1.0, Vec3::new(0.93, 0.95, 0.97)),
                ],
                nubes: 0.48,
                velocidad_nubes: 0.013,
                color_atmosfera: Vec3::new(0.28, 0.52, 1.0),
            }),
            normal_map: Some(nmap),
            atmosphere: Some(Atmosphere {
                color: Vec3::new(0.26, 0.48, 0.98),
                thickness: 0.055,
                intensity: 0.60,
            }),
            rings: None,
            prop_batches: fusionar_props(&props),
            props,
            mass: 42.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // 3. SELENE: luna de TERRANOVA.
        let terreno = Terrain::new(TerrainKind::Craters, 0.07, 5.0, seed ^ 0x23);
        let nmap = Arc::new(generate_normal_map(&terreno, 256, 128, 1.8));
        bodies.push(Body {
            name: "SELENE".to_string(),
            radius: 0.42,
            orbit: Orbit { radius: 6.0, speed: 0.42, phase: 0.0, inclination: to_radians(9.0) },
            parent: Some(2),
            spin_speed: 0.62,
            axial_tilt: 0.0,
            terrain: terreno,
            lods: construir_lods(0.42, &terreno, &LODS_LUNA),
            shader: Box::new(RockyShader {
                noise: Noise::new(seed ^ 0x2c2),
                normal_map: nmap.clone(),
                normal_strength: 1.7,
                paleta: vec![
                    (0.0, Vec3::new(0.14, 0.14, 0.16)),
                    (0.5, Vec3::new(0.38, 0.38, 0.40)),
                    (1.0, Vec3::new(0.70, 0.70, 0.72)),
                ],
                detalle: 24.0,
                especular: 0.02,
            }),
            normal_map: Some(nmap),
            atmosphere: None,
            rings: None,
            props: Vec::new(),
            prop_batches: Vec::new(),
            mass: 3.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // ---------------------------------------------------------------
        // 4. AURORA: gigante gaseoso con anillos.
        // ---------------------------------------------------------------
        let radio = 3.3;
        let terreno = Terrain::flat(seed ^ 0x33);
        bodies.push(Body {
            name: "AURORA".to_string(),
            radius: radio,
            orbit: Orbit { radius: 54.0, speed: 0.052, phase: 4.1, inclination: 0.0 },
            parent: Some(0),
            spin_speed: 0.47,
            axial_tilt: to_radians(12.0),
            terrain: terreno,
            lods: construir_lods(radio, &terreno, &LODS_PLANETA),
            shader: Box::new(GasShader {
                noise: Noise::new(seed ^ 0x3d3),
                bandas: 11.0,
                deriva: 0.035,
                turbulencia: 0.55,
                color_claro: Vec3::new(0.88, 0.76, 0.56),
                color_oscuro: Vec3::new(0.40, 0.26, 0.19),
                color_tormenta: Vec3::new(0.86, 0.33, 0.20),
                tormenta_lat: -0.28,
                tormenta_lon: 1.3,
                tormenta_tam: 0.55,
                color_atmosfera: Vec3::new(0.82, 0.70, 0.48),
            }),
            normal_map: None,
            atmosphere: Some(Atmosphere {
                color: Vec3::new(0.85, 0.72, 0.48),
                thickness: 0.04,
                intensity: 0.6,
            }),
            rings: Some(RingSystem {
                mesh: ring(radio * 1.45, radio * 2.5, 192),
                shader: Box::new(RingShader {
                    noise: Noise::new(seed ^ 0x3e3),
                    color_interno: Vec3::new(0.92, 0.82, 0.64),
                    color_externo: Vec3::new(0.52, 0.46, 0.42),
                    frecuencia: 26.0,
                    hueco: 0.58,
                    densidad: 1.0,
                }),
                outer_radius: radio * 2.5,
            }),
            props: Vec::new(),
            prop_batches: Vec::new(),
            mass: 160.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // 5. IO MENOR: luna volcanica de AURORA.
        let terreno = Terrain::new(TerrainKind::Mountains, 0.08, 6.0, seed ^ 0x34);
        let nmap = Arc::new(generate_normal_map(&terreno, 256, 128, 1.6));
        bodies.push(Body {
            name: "IO MENOR".to_string(),
            radius: 0.5,
            orbit: Orbit { radius: 9.5, speed: 0.30, phase: 1.0, inclination: to_radians(4.0) },
            parent: Some(4),
            spin_speed: 0.5,
            axial_tilt: 0.0,
            terrain: terreno,
            lods: construir_lods(0.5, &terreno, &LODS_LUNA),
            shader: Box::new(RockyShader {
                noise: Noise::new(seed ^ 0x4a4),
                normal_map: nmap.clone(),
                normal_strength: 1.6,
                paleta: vec![
                    (0.0, Vec3::new(0.24, 0.12, 0.06)),
                    (0.5, Vec3::new(0.62, 0.40, 0.12)),
                    (1.0, Vec3::new(0.92, 0.80, 0.35)),
                ],
                detalle: 28.0,
                especular: 0.08,
            }),
            normal_map: Some(nmap),
            atmosphere: None,
            rings: None,
            props: Vec::new(),
            prop_batches: Vec::new(),
            mass: 4.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // ---------------------------------------------------------------
        // 6. GELIDA: mundo helado con bosques de cristal.
        // ---------------------------------------------------------------
        let terreno = Terrain::new(TerrainKind::IcePlates, 0.05, 2.8, seed ^ 0x44);
        let nmap = Arc::new(generate_normal_map(&terreno, 512, 256, 1.4));
        let radio = 1.45;
        let recipe = PropRecipe {
            seed: seed ^ 0xC01D,
            attempts: 7000,
            max_instances: 190,
            min_separation: 0.075,
            cluster_scale: 3.1,
            cluster_threshold: 0.10,
            min_height: -0.30,
            weights: [0.0, 1.2, 5.0, 0.8],
            scale_range: (0.6, 1.6),
            tints: [
                Vec3::ONE,
                Vec3::new(0.78, 0.84, 0.95),
                Vec3::new(0.70, 0.92, 1.25),
                Vec3::new(0.80, 0.88, 1.00),
            ],
        };
        let props = scatter(&recipe, &terreno, radio);
        bodies.push(Body {
            name: "GELIDA".to_string(),
            radius: radio,
            orbit: Orbit { radius: 76.0, speed: 0.034, phase: 5.6, inclination: 0.0 },
            parent: Some(0),
            spin_speed: 0.21,
            axial_tilt: to_radians(27.0),
            terrain: terreno,
            lods: construir_lods(radio, &terreno, &LODS_PLANETA),
            shader: Box::new(IceShader {
                noise: Noise::new(seed ^ 0x5e5),
                normal_map: nmap.clone(),
                normal_strength: 1.2,
                escala_placas: 7.0,
                hielo_claro: Vec3::new(0.80, 0.90, 0.99),
                hielo_sombra: Vec3::new(0.38, 0.52, 0.70),
                color_grieta: Vec3::new(0.05, 0.14, 0.30),
                color_atmosfera: Vec3::new(0.45, 0.70, 1.0),
            }),
            normal_map: Some(nmap),
            atmosphere: Some(Atmosphere {
                color: Vec3::new(0.45, 0.70, 1.0),
                thickness: 0.045,
                intensity: 0.7,
            }),
            rings: None,
            prop_batches: fusionar_props(&props),
            props,
            mass: 30.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // ---------------------------------------------------------------
        // 7. VULCANO: cordilleras y monumentos.
        // ---------------------------------------------------------------
        let terreno = Terrain::new(TerrainKind::Mountains, 0.075, 2.6, seed ^ 0x55);
        let nmap = Arc::new(generate_normal_map(&terreno, 512, 256, 1.5));
        let radio = 1.9;
        let recipe = PropRecipe {
            seed: seed ^ 0xF17E,
            attempts: 8000,
            max_instances: 170,
            min_separation: 0.09,
            cluster_scale: 2.0,
            cluster_threshold: 0.14,
            min_height: 0.0,
            weights: [0.0, 3.0, 1.0, 2.6],
            scale_range: (0.7, 1.5),
            tints: [
                Vec3::ONE,
                Vec3::new(0.95, 0.62, 0.48),
                Vec3::new(1.25, 0.55, 0.35),
                Vec3::new(0.85, 0.78, 0.74),
            ],
        };
        let props = scatter(&recipe, &terreno, radio);
        bodies.push(Body {
            name: "VULCANO".to_string(),
            radius: radio,
            orbit: Orbit { radius: 98.0, speed: 0.022, phase: 3.2, inclination: 0.0 },
            parent: Some(0),
            spin_speed: 0.17,
            axial_tilt: to_radians(32.0),
            terrain: terreno,
            lods: construir_lods(radio, &terreno, &LODS_PLANETA),
            shader: Box::new(RockyShader {
                noise: Noise::new(seed ^ 0x6f6),
                normal_map: nmap.clone(),
                normal_strength: 1.6,
                paleta: vec![
                    (0.0, Vec3::new(0.10, 0.07, 0.07)),
                    (0.35, Vec3::new(0.28, 0.12, 0.09)),
                    (0.62, Vec3::new(0.52, 0.22, 0.12)),
                    (0.85, Vec3::new(0.40, 0.33, 0.31)),
                    (1.0, Vec3::new(0.80, 0.76, 0.74)),
                ],
                detalle: 14.0,
                especular: 0.07,
            }),
            normal_map: Some(nmap),
            atmosphere: Some(Atmosphere {
                color: Vec3::new(0.95, 0.42, 0.22),
                thickness: 0.05,
                intensity: 0.55,
            }),
            rings: None,
            prop_batches: fusionar_props(&props),
            props,
            mass: 48.0,
            emissive: false,
            position: Vec3::ZERO,
            spin: 0.0,
        });

        // Lineas de orbita de los cuerpos que giran alrededor de la
        // estrella. Dejan ver de un vistazo que todos comparten el plano
        // ecliptico.
        let mut orbit_meshes = Vec::new();
        for (i, b) in bodies.iter().enumerate() {
            if b.parent == Some(0) {
                let r = b.orbit.radius;
                orbit_meshes.push((i, ring(r - 0.05, r + 0.05, 256)));
            }
        }

        let prop_shaders = vec![
            // Arbol
            PropShader {
                noise: Noise::new(seed ^ 0x7001),
                base: Vec3::new(0.10, 0.21, 0.09),
                alterno: Vec3::new(0.24, 0.42, 0.14),
                veta: 6.0,
                especular: 0.02,
                emision: Vec3::ZERO,
            },
            // Roca
            PropShader {
                noise: Noise::new(seed ^ 0x7002),
                base: Vec3::new(0.19, 0.17, 0.16),
                alterno: Vec3::new(0.42, 0.38, 0.35),
                veta: 9.0,
                especular: 0.04,
                emision: Vec3::ZERO,
            },
            // Cristal
            PropShader {
                noise: Noise::new(seed ^ 0x7003),
                base: Vec3::new(0.10, 0.26, 0.38),
                alterno: Vec3::new(0.35, 0.70, 0.95),
                veta: 4.0,
                especular: 1.1,
                emision: Vec3::new(0.06, 0.18, 0.30),
            },
            // Torre
            PropShader {
                noise: Noise::new(seed ^ 0x7004),
                base: Vec3::new(0.30, 0.28, 0.26),
                alterno: Vec3::new(0.58, 0.55, 0.50),
                veta: 11.0,
                especular: 0.22,
                emision: Vec3::new(0.05, 0.03, 0.01),
            },
        ];

        Self {
            bodies,
            skybox: Skybox::new(seed ^ 0xBEEF, 2600),
            prop_shaders,
            ship_mesh: models::ship(),
            ship_shader: ShipShader {
                noise: Noise::new(seed ^ 0x8001),
                casco: Vec3::new(0.46, 0.48, 0.54),
                franja: Vec3::new(0.85, 0.26, 0.12),
                cabina: Vec3::new(0.25, 0.65, 0.95),
            },
            flare_mesh: models::engine_flare(),
            flare_shader: GlowShader {
                color: Vec3::new(0.35, 0.65, 1.30),
                potencia: 1.4,
                intensidad: 1.0,
                solido: true,
            },
            atmosphere_mesh: icosphere(1.0, 3),
            orbit_meshes,
            orbit_shader: TintShader {
                color: Vec3::new(0.22, 0.30, 0.50),
                alpha: 0.55,
                distancia_visible: 70.0,
            },
            corona_mesh: icosphere(1.0, 3),
            corona_shader: CoronaShader {
                noise: Noise::new(seed ^ 0x9001),
                color: Vec3::new(1.80, 0.60, 0.14),
                flow: 0.07,
                escala: 1.55,
            },
            time: 0.0,
        }
    }

    /// Avanza el sistema `dt` segundos: orbitas y giros propios.
    ///
    /// Todo se calcula a partir del tiempo acumulado, de modo que la
    /// simulacion va igual de rapido sin importar cuantos cuadros por
    /// segundo logre la maquina.
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        let t = self.time;

        for i in 0..self.bodies.len() {
            let local = self.bodies[i].orbit.position(t);
            let base = match self.bodies[i].parent {
                Some(p) => {
                    debug_assert!(p < i, "un cuerpo debe declararse despues de su padre");
                    self.bodies[p].position
                }
                None => Vec3::ZERO,
            };
            self.bodies[i].position = base + local;
            self.bodies[i].spin = (self.bodies[i].spin_speed * t) % TAU;
        }
    }

    /// Indice del cuerpo mas cercano a un punto, contando la estrella.
    pub fn mas_cercano_total(&self, p: Vec3) -> usize {
        let mut mejor = 0;
        let mut mejor_d = f32::INFINITY;
        for (i, b) in self.bodies.iter().enumerate() {
            let d = (b.position - p).length() - b.outer_radius();
            if d < mejor_d {
                mejor_d = d;
                mejor = i;
            }
        }
        mejor
    }

    /// Indice del cuerpo mas cercano a un punto, sin contar la estrella.
    pub fn cuerpo_mas_cercano(&self, p: Vec3) -> usize {
        let mut mejor = 1;
        let mut mejor_d = f32::INFINITY;
        for (i, b) in self.bodies.iter().enumerate().skip(1) {
            let d = (b.position - p).length() - b.outer_radius();
            if d < mejor_d {
                mejor_d = d;
                mejor = i;
            }
        }
        mejor
    }

    /// Altura de un punto sobre la superficie del cuerpo indicado.
    pub fn altura_sobre(&self, indice: usize, p: Vec3) -> f32 {
        let b = &self.bodies[indice];
        let d = p - b.position;
        let dist = d.length();
        if dist < 1e-4 {
            return -b.radius;
        }
        // La direccion se lleva al espacio del planeta para consultar el
        // relieve en el punto correcto mientras el planeta gira.
        let inv = b.model_matrix().inverse_rigid();
        let dir_local = inv.mul_dir(d).normalized();
        dist - b.surface_radius(dir_local)
    }

    /// Aplica gravedad y colisiones a la nave. Devuelve si hubo choque.
    pub fn aplicar_fisica(&self, ship: &mut Ship, dt: f32, gravedad: bool) -> bool {
        if gravedad {
            for b in &self.bodies {
                ship.apply_gravity(b.position, b.mass, dt, G);
            }
        }
        let mut choco = false;
        for b in &self.bodies {
            let d = ship.position - b.position;
            let dist = d.length();
            // Solo se consulta el relieve de los cuerpos que estan cerca.
            if dist > b.outer_radius() + ship.radius + 1.0 {
                continue;
            }
            let inv = b.model_matrix().inverse_rigid();
            let dir_local = inv.mul_dir(d).normalized();
            let superficie = b.surface_radius(dir_local);
            if ship.resolve_collision(b.position, superficie) {
                choco = true;
            }
        }
        choco
    }

    /// Dibuja la escena completa.
    pub fn draw(
        &self,
        fb: &mut Framebuffer,
        camera: &Camera,
        ship: &Ship,
        mostrar_nave: bool,
        toggles: &RenderToggles,
        stats: &RasterStats,
    ) {
        stats.reset();

        let aspect = fb.aspect();
        let view = camera.view();
        let projection = camera.projection(aspect);
        let view_projection = projection * view;
        let frustum = Frustum::from_view_projection(&view_projection);

        // 1. Fondo: nebulosa y estrellas.
        self.skybox.draw_background(fb, camera.basis(), camera.fov_y);
        self.skybox.draw_stars(fb, &view_projection);

        let base = Uniforms {
            view,
            projection,
            light_world: self.bodies[0].position,
            camera_world: camera.position,
            time: self.time,
            ..Default::default()
        };

        // 2. Cuerpos opacos, con descarte por frustum y nivel de detalle.
        for b in &self.bodies {
            if toggles.frustum_culling
                && !frustum.sphere_visible(b.position, b.outer_radius() * 1.05)
            {
                stats.meshes_culled.fetch_add(1, Ordering::Relaxed);
                continue;
            }

            let model = b.model_matrix();
            let u = Uniforms {
                model,
                normal_matrix: model.normal_matrix(),
                object_scale: b.radius,
                ..base
            };
            let lod = b.lod_index(camera.position, toggles.lod);
            draw_mesh(fb, &b.lods[lod], b.shader.as_ref(), &u, DrawOptions::opaque(), stats);

            // 3. Objetos de superficie, solo cuando el planeta esta cerca.
            if toggles.props && !b.prop_batches.is_empty() {
                let distancia = (camera.position - b.position).length();
                if distancia < b.radius * 16.0 {
                    for batch in &b.prop_batches {
                        let up = Uniforms { tint: batch.tint, ..u };
                        draw_mesh(
                            fb,
                            &batch.mesh,
                            &self.prop_shaders[batch.kind],
                            &up,
                            DrawOptions::opaque(),
                            stats,
                        );
                    }
                }
            }

            // 4. Anillos: comparten la inclinacion del planeta pero no su
            //    giro.
            if let Some(r) = &b.rings {
                if !toggles.frustum_culling
                    || frustum.sphere_visible(b.position, r.outer_radius * 1.05)
                {
                    let model = b.frame_matrix();
                    let u = Uniforms { model, normal_matrix: model.normal_matrix(), ..base };
                    draw_mesh(fb, &r.mesh, r.shader.as_ref(), &u, DrawOptions::additive(), stats);
                }
            }
        }

        // 5. Corona de la estrella.
        {
            let sol = &self.bodies[0];
            let model = Mat4::translation(sol.position) * Mat4::uniform_scale(sol.radius * 1.55);
            let u = Uniforms { model, normal_matrix: model.normal_matrix(), ..base };
            draw_mesh(fb, &self.corona_mesh, &self.corona_shader, &u, DrawOptions::additive(), stats);
        }

        // 6. Atmosferas.
        if toggles.atmosferas {
            for b in &self.bodies {
                let Some(a) = b.atmosphere else { continue };
                let escala = b.outer_radius() * (1.0 + a.thickness);
                if toggles.frustum_culling && !frustum.sphere_visible(b.position, escala) {
                    continue;
                }
                let model = Mat4::translation(b.position) * Mat4::uniform_scale(escala);
                let u = Uniforms { model, normal_matrix: model.normal_matrix(), ..base };
                let shader = AtmosphereShader {
                    color: a.color,
                    potencia: 2.6,
                    intensidad: a.intensity,
                };
                draw_mesh(fb, &self.atmosphere_mesh, &shader, &u, DrawOptions::additive(), stats);
            }
        }

        // 7. Lineas de orbita.
        if toggles.orbitas {
            let u = Uniforms {
                model: Mat4::identity(),
                normal_matrix: Mat4::identity(),
                ..base
            };
            for (_, mesh) in &self.orbit_meshes {
                draw_mesh(fb, mesh, &self.orbit_shader, &u, DrawOptions::additive(), stats);
            }
        }

        // 8. La nave y la llama de sus motores.
        if mostrar_nave {
            let model = ship.matrix();
            let u = Uniforms { model, normal_matrix: model.normal_matrix(), ..base };
            draw_mesh(fb, &self.ship_mesh, &self.ship_shader, &u, DrawOptions::opaque(), stats);

            let empuje = ship.throttle.max(0.0);
            if empuje > 0.02 {
                let largo = 0.5 + empuje * 1.4;
                let model = ship.matrix() * Mat4::scale(Vec3::new(1.0, 1.0, largo));
                let shader = GlowShader {
                    color: self.flare_shader.color * (0.5 + empuje),
                    potencia: self.flare_shader.potencia,
                    intensidad: (0.35 + empuje * 0.9).min(1.0),
                    solido: true,
                };
                let u = Uniforms { model, normal_matrix: model.normal_matrix(), ..base };
                draw_mesh(fb, &self.flare_mesh, &shader, &u, DrawOptions::additive(), stats);
            }
        }
    }
}

/// Genera los niveles de detalle de un cuerpo, todos con el mismo campo de
/// altura para que el cambio de nivel no produzca un salto visible.
fn construir_lods(radio: f32, terreno: &Terrain, niveles: &[u32]) -> Vec<Mesh> {
    niveles
        .iter()
        .map(|&n| {
            let mut m = icosphere(radio, n);
            displace_sphere(&mut m, terreno, radio);
            m
        })
        .collect()
}

/// Fusiona todas las copias de cada modelo de superficie en una sola
/// malla, en espacio del planeta.
///
/// Cada instancia se transforma con su matriz local -la que la para sobre
/// la curva del planeta- y su parametro se escribe en el canal libre del
/// vertice, para que el shader pueda variar el color copia por copia sin
/// necesidad de una llamada de dibujo por objeto.
fn fusionar_props(props: &[PropInstance]) -> Vec<PropBatch> {
    let modelos = models::surface_props();
    let mut batches: Vec<PropBatch> = Vec::new();

    for (kind, modelo) in modelos.iter().enumerate() {
        let instancias: Vec<&PropInstance> = props.iter().filter(|p| p.kind == kind).collect();
        if instancias.is_empty() {
            continue;
        }
        let mut acumulada = Mesh::default();
        let mut tinte = Vec3::ZERO;
        for inst in &instancias {
            let mut copia = modelo.clone();
            copia.transform(&inst.local_matrix());
            for v in &mut copia.vertices {
                v.height = inst.param;
            }
            acumulada.merge(&copia);
            tinte += inst.tint;
        }
        tinte = tinte / instancias.len() as f32;
        batches.push(PropBatch { kind, mesh: acumulada, tint: tinte });
    }

    batches
}
