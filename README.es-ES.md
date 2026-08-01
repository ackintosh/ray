

<h1 align="center">Ray</h1>

_Ray_ es una implementación de nodo Beacon de Ethereum de código abierto diseñada con fines educativos. Ofrece una experiencia de aprendizaje práctica para quienes desean comprender los matices de la implementación de un nodo Beacon.

[![CI](https://github.com/ackintosh/ray/actions/workflows/ci.yml/badge.svg)](https://github.com/ackintosh/ray/actions/workflows/ci.yml)

![banner image](https://raw.githubusercontent.com/ackintosh/ray/898488c66bf520a5df71a8d28c562b12355af9ee/banner.jpeg)

> Foto de [Marc-Olivier Jodoin](https://unsplash.com/@marcojodoin?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText) en [Unsplash](https://unsplash.com/?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText)

## Visión general

El proyecto `Ray` ofrece a los desarrolladores la oportunidad de aprender a implementar un nodo Beacon mediante su ejecución o la lectura del código fuente. Nos centramos en la _red (networking)_, por lo que algunos componentes necesarios para implementar un BeaconNode (p. ej., BeaconChain) se han tomado de [lighthouse](https://github.com/sigp/lighthouse), un cliente de consenso de Ethereum escrito en Rust.

Ray no está listo para producción, pero debería ser suficiente para fines de aprendizaje.

NOTA: Ray está en desarrollo activo.

Buscamos una implementación simple, limitando las funciones. 

- Disponible únicamente para macOS
- Se ejecuta en la red de pruebas _Prater_
  - La red de pruebas Görli se ha fusionado con la cadena Beacon de prueba de participación (PoS) de Prater.
  - https://github.com/eth-clients/goerli

### Estado actual

- [x] [Descubrimiento de nodos](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#the-discovery-domain-discv5)
- RPC
  - [x] [Status](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status)
  - [x] [Goodbye](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye)
  - [ ] BeaconBlocksByRange
  - [ ] BeaconBlocksByRoot
  - [ ] Ping
  - [ ] GetMetaData
- [ ] Sincronizar la Beacon Chain

## Primeros pasos

### Ejecutar `ray`

Necesitas una cadena de herramientas de Rust reciente para comenzar. Si aún no la tienes, consulta [Instalar Rust](https://www.rust-lang.org/tools/install). Una vez hecho esto, simplemente puedes usar `cargo` como se indica a continuación.

```shell
git clone https://github.com/ackintosh/ray.git
cd ray
RUST_LOG=ray=info cargo run
```

### Arquitectura

El siguiente diagrama muestra los módulos y cómo funcionan. Esto te ayudará a obtener una comprensión general de Ray y a facilitar la lectura del código.

![Architecture](https://raw.githubusercontent.com/ackintosh/ray/main/diagrams/architecture.png)

## Referencias para implementadores de nodos Beacon

Aquí se encuentran las especificaciones y documentaciones a las que deben consultar los implementadores de la Capa de Consenso.

### Especificaciones

- https://github.com/ethereum/consensus-specs
- https://github.com/ethereum/devp2p
- https://github.com/ethereum/beacon-metrics

### Documentos

- https://github.com/timbeiko/eth-roadmap-faq

### Videos

- [Serie Whiteboard con NEAR | Ep: 44 Adrian Manning de Sigma Prime](https://www.youtube.com/watch?v=XvWf6QMBO6k)  
  - > Exploraremos cómo se comunican los nodos y por qué lo hacen. Esto incluirá Discovery v5, libp2p, Gossipsub y el RPC de Eth2.

### Canales de YouTube

- [Infinite Jungle](https://www.youtube.com/playlist?list=PLz3vbkrzRoXR_XIWVqnZcX11REeC6acN2) de Galaxy es un podcast sobre la evolución de Ethereum. Presentado semanalmente por Christine Kim de Galaxy Research, el programa se adentra en las formas en que Ethereum como ecosistema está cambiando, cubriendo tendencias y narrativas amplias que están moldeando la forma en que las personas usan Ethereum y piensan sobre su valor.


## Autor

Creado y mantenido por ackintosh.

> GitHub [@ackintosh](https://github.com/ackintosh) / Twitter [@NAKANO_Akihito](https://twitter.com/NAKANO_Akihito)
