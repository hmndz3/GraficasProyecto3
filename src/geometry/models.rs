//! Modelos propios: la nave y los cuatro tipos de objeto que se siembran
//! sobre la superficie de los planetas.
//!
//! Cada modelo se arma uniendo primitivas transformadas. El convenio de
//! ejes es: +Z hacia adelante, +Y hacia arriba.

use super::mesh::Mesh;
use super::primitives::{bipyramid, box_mesh, cone, cylinder};
use super::sphere::icosphere;
use crate::math::{Mat4, Vec3, PI};

/// Pega una pieza transformada dentro de la malla que se esta armando.
fn attach(target: &mut Mesh, mut piece: Mesh, transform: Mat4) {
    piece.transform(&transform);
    target.merge(&piece);
}

/// Nave del jugador.
///
/// Fuselaje alargado, nariz conica, dos alas en delta, una cabina y dos
/// motores traseros. Mide alrededor de 2.6 unidades de largo y esta
/// centrada en el origen.
pub fn ship() -> Mesh {
    let mut m = Mesh::default();

    // Fuselaje: cilindro conico acostado sobre el eje Z.
    attach(
        &mut m,
        cylinder(0.30, 0.16, 1.8, 10),
        Mat4::translation(Vec3::new(0.0, 0.0, 0.0)) * Mat4::rotation_x(PI * 0.5),
    );

    // Nariz.
    attach(
        &mut m,
        cone(0.30, 0.70, 10),
        Mat4::translation(Vec3::new(0.0, 0.0, 0.90)) * Mat4::rotation_x(PI * 0.5),
    );

    // Cola.
    attach(
        &mut m,
        cylinder(0.16, 0.22, 0.25, 10),
        Mat4::translation(Vec3::new(0.0, 0.0, -1.0)) * Mat4::rotation_x(PI * 0.5),
    );

    // Alas: cajas aplanadas e inclinadas hacia atras.
    for side in [-1.0f32, 1.0] {
        attach(
            &mut m,
            box_mesh(Vec3::new(0.62, 0.035, 0.34)),
            Mat4::translation(Vec3::new(side * 0.62, -0.02, -0.28))
                * Mat4::rotation_z(side * -0.22)
                * Mat4::rotation_y(side * 0.35),
        );
        // Aleta vertical en la punta del ala.
        attach(
            &mut m,
            box_mesh(Vec3::new(0.03, 0.16, 0.20)),
            Mat4::translation(Vec3::new(side * 1.18, 0.12, -0.34)),
        );
    }

    // Aleta dorsal.
    attach(
        &mut m,
        box_mesh(Vec3::new(0.03, 0.30, 0.28)),
        Mat4::translation(Vec3::new(0.0, 0.28, -0.70)),
    );

    // Cabina: media esfera aplastada sobre el lomo.
    attach(
        &mut m,
        icosphere(1.0, 1),
        Mat4::translation(Vec3::new(0.0, 0.20, 0.25)) * Mat4::scale(Vec3::new(0.21, 0.14, 0.38)),
    );

    // Motores.
    for side in [-1.0f32, 1.0] {
        attach(
            &mut m,
            cylinder(0.13, 0.17, 0.55, 10),
            Mat4::translation(Vec3::new(side * 0.34, -0.06, -0.85)) * Mat4::rotation_x(PI * 0.5),
        );
    }

    m.update_bounds();
    m
}

/// Disco de la llama del motor, dibujado con mezcla aditiva detras de la
/// nave.
pub fn engine_flare() -> Mesh {
    let mut m = Mesh::default();
    for side in [-1.0f32, 1.0] {
        attach(
            &mut m,
            cone(0.15, 0.9, 8),
            Mat4::translation(Vec3::new(side * 0.34, -0.06, -1.15)) * Mat4::rotation_x(-PI * 0.5),
        );
    }
    m
}

/// Tipo 1: arbol. Tronco cilindrico y tres copas conicas escalonadas.
pub fn tree() -> Mesh {
    let mut m = Mesh::default();
    attach(
        &mut m,
        cylinder(0.055, 0.04, 0.42, 7),
        Mat4::translation(Vec3::new(0.0, 0.21, 0.0)),
    );
    let tiers = [(0.30f32, 0.34f32, 0.36f32), (0.24, 0.30, 0.58), (0.16, 0.26, 0.78)];
    for (r, h, y) in tiers {
        attach(&mut m, cone(r, h, 8), Mat4::translation(Vec3::new(0.0, y, 0.0)));
    }
    m
}

/// Tipo 2: roca. Icoesfera de nivel bajo, escalada de forma irregular y
/// girada para que ninguna copia se vea igual.
pub fn rock() -> Mesh {
    let mut m = icosphere(0.28, 1);
    // Se achata y se deforman los vertices para quitarle la cara de
    // esfera.
    for (i, v) in m.vertices.iter_mut().enumerate() {
        let k = 0.78 + ((i * 2654435761) % 97) as f32 / 97.0 * 0.45;
        v.position = v.position * k;
    }
    m.transform(&Mat4::scale(Vec3::new(1.15, 0.72, 1.0)));
    m.transform(&Mat4::translation(Vec3::new(0.0, 0.16, 0.0)));
    m.recompute_normals();
    m
}

/// Tipo 3: cristal. Un prisma central alto y dos esquirlas inclinadas.
pub fn crystal() -> Mesh {
    let mut m = Mesh::default();
    attach(
        &mut m,
        bipyramid(0.11, 0.95, 6),
        Mat4::translation(Vec3::new(0.0, 0.42, 0.0)),
    );
    attach(
        &mut m,
        bipyramid(0.07, 0.52, 5),
        Mat4::translation(Vec3::new(0.14, 0.24, 0.05)) * Mat4::rotation_z(-0.42),
    );
    attach(
        &mut m,
        bipyramid(0.06, 0.40, 5),
        Mat4::translation(Vec3::new(-0.12, 0.19, -0.08)) * Mat4::rotation_z(0.5),
    );
    m
}

/// Tipo 4: torre monumento. Base ancha, fuste escalonado y remate.
pub fn tower() -> Mesh {
    let mut m = Mesh::default();
    attach(
        &mut m,
        box_mesh(Vec3::new(0.26, 0.05, 0.26)),
        Mat4::translation(Vec3::new(0.0, 0.05, 0.0)),
    );
    attach(
        &mut m,
        box_mesh(Vec3::new(0.17, 0.30, 0.17)),
        Mat4::translation(Vec3::new(0.0, 0.40, 0.0)),
    );
    attach(
        &mut m,
        box_mesh(Vec3::new(0.11, 0.26, 0.11)),
        Mat4::translation(Vec3::new(0.0, 0.94, 0.0)) * Mat4::rotation_y(PI * 0.25),
    );
    attach(
        &mut m,
        cone(0.15, 0.40, 4),
        Mat4::translation(Vec3::new(0.0, 1.20, 0.0)) * Mat4::rotation_y(PI * 0.25),
    );
    // Antena.
    attach(
        &mut m,
        cylinder(0.012, 0.012, 0.30, 5),
        Mat4::translation(Vec3::new(0.0, 1.70, 0.0)),
    );
    m
}

/// Los cuatro tipos de objeto de superficie, en orden.
pub fn surface_props() -> [Mesh; 4] {
    [tree(), rock(), crystal(), tower()]
}
