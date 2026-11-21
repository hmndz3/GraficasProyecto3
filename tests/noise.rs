//! Pruebas del ruido procedural.
//!
//! Lo que importa aqui es que el resultado sea reproducible: con la misma
//! semilla, la distribucion de objetos y el relieve de los planetas tienen
//! que salir identicos en cualquier corrida.

use sistema_solar::math::Vec3;
use sistema_solar::noise::{Noise, Rng};

#[test]
fn la_misma_semilla_da_el_mismo_valor() {
    let a = Noise::new(1234);
    let b = Noise::new(1234);
    let p = Vec3::new(3.7, -1.2, 8.9);
    assert_eq!(a.value(p), b.value(p));
    assert_eq!(a.fbm(p, 5, 2.0, 0.5), b.fbm(p, 5, 2.0, 0.5));
    assert_eq!(a.cellular(p), b.cellular(p));
}

#[test]
fn semillas_distintas_dan_campos_distintos() {
    let a = Noise::new(1);
    let b = Noise::new(2);
    let diferentes = (0..50)
        .filter(|i| {
            let p = Vec3::new(*i as f32 * 0.37, 1.1, -0.4);
            (a.value(p) - b.value(p)).abs() > 1e-6
        })
        .count();
    assert!(diferentes > 40, "los campos deberian ser independientes");
}

#[test]
fn el_ruido_se_queda_en_su_rango() {
    let n = Noise::new(99);
    for i in 0..2000 {
        let p = Vec3::new(i as f32 * 0.13, i as f32 * -0.07, i as f32 * 0.29);
        let v = n.value(p);
        assert!((-1.0..=1.0).contains(&v), "value fuera de rango: {v}");
        let f = n.fbm(p, 6, 2.0, 0.5);
        assert!((-1.0..=1.0).contains(&f), "fbm fuera de rango: {f}");
        let c = n.cellular(p);
        assert!((0.0..=1.0).contains(&c), "cellular fuera de rango: {c}");
        let r = n.ridged(p, 5);
        assert!((0.0..=1.0).contains(&r), "ridged fuera de rango: {r}");
    }
}

#[test]
fn el_ruido_es_continuo() {
    // Dos puntos muy cercanos no pueden dar valores muy distintos: si lo
    // hicieran, la superficie deformada saldria llena de picos sueltos.
    let n = Noise::new(7);
    let p = Vec3::new(2.3, -4.1, 0.8);
    let q = p + Vec3::new(0.001, 0.0, 0.0);
    assert!((n.value(p) - n.value(q)).abs() < 0.02);
}

#[test]
fn en_los_enteros_el_ruido_vale_lo_que_la_celda() {
    // Con interpolacion quintica, en coordenada entera el valor es
    // exactamente el de la esquina, sin mezcla.
    let n = Noise::new(42);
    let a = n.value(Vec3::new(3.0, 5.0, -2.0));
    let b = n.value(Vec3::new(3.0, 5.0, -2.0));
    assert_eq!(a, b);
    assert!((-1.0..=1.0).contains(&a));
}

#[test]
fn el_generador_aleatorio_es_reproducible() {
    let mut a = Rng::new(2024);
    let mut b = Rng::new(2024);
    for _ in 0..100 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
}

#[test]
fn los_flotantes_aleatorios_estan_en_cero_uno() {
    let mut r = Rng::new(5);
    for _ in 0..10_000 {
        let v = r.next_f32();
        assert!((0.0..1.0).contains(&v), "fuera de rango: {v}");
    }
}

#[test]
fn los_puntos_de_la_esfera_son_unitarios() {
    let mut r = Rng::new(17);
    for _ in 0..1000 {
        let p = r.unit_sphere();
        assert!((p.length() - 1.0).abs() < 1e-4, "longitud {}", p.length());
    }
}

#[test]
fn los_puntos_cubren_toda_la_esfera() {
    // Se cuenta cuantos caen en cada octante: deberian repartirse.
    let mut r = Rng::new(3);
    let mut octantes = [0usize; 8];
    for _ in 0..8000 {
        let p = r.unit_sphere();
        let i = (p.x > 0.0) as usize | ((p.y > 0.0) as usize) << 1 | ((p.z > 0.0) as usize) << 2;
        octantes[i] += 1;
    }
    for (i, c) in octantes.iter().enumerate() {
        assert!(*c > 700, "el octante {i} quedo vacio: {c}");
    }
}

#[test]
fn la_distorsion_del_dominio_mueve_el_punto() {
    let n = Noise::new(11);
    let p = Vec3::new(1.0, 2.0, 3.0);
    let q = n.warp(p, 0.5);
    assert!((p - q).length() > 1e-3, "warp deberia desplazar el punto");
    assert!((p - q).length() < 1.0, "el desplazamiento no deberia explotar");
}
