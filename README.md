# Ray

_Ray_ is an open-source Ethereum Beacon Node implementation crafted for educational purposes. It offers a hands-on learning experience for those looking to understand the intricacies of Beacon Node implementation.

[![CI](https://github.com/ackintosh/ray/actions/workflows/ci.yml/badge.svg)](https://github.com/ackintosh/ray/actions/workflows/ci.yml)

![banner image](https://raw.githubusercontent.com/ackintosh/ray/898488c66bf520a5df71a8d28c562b12355af9ee/banner.jpeg)

> Photo by [Marc-Olivier Jodoin](https://unsplash.com/@marcojodoin?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText) on [Unsplash](https://unsplash.com/?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText)

## Overview

This project `Ray` provides developers with an opportunity to learn how to implement Beacon Node via running the node or reading the source codes. We're focusing to _networking_, so some components needed to implement a BeaconNode (e.g. BeaconChain) are borrowed from [lighthouse](https://github.com/sigp/lighthouse), which is an Ethereum consensus client in Rust.

Ray is never production ready but should be enough to learn from.

NOTE: Ray is under active development.

We aim for a simple implementation, by narrowing down the functions. 

- Available on only macOS
- Runs for _Prater_ testnet
  - Görli testnet has been merged with the Prater proof-of-stake beacon chain.
  - https://github.com/eth-clients/goerli

### Current status

- [x] [Node discovery](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#the-discovery-domain-discv5)
- RPC
  - [x] [Status](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status)
  - [x] [Goodbye](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#goodbye)
  - [ ] BeaconBlocksByRange
  - [ ] BeaconBlocksByRoot
  - [ ] Ping
  - [ ] GetMetaData
- [ ] Sync the Beacon Chain

## Getting started

### Running `ray`

You need a recent Rust toolchain to get started. If you don't have one already, check out [Install Rust](https://www.rust-lang.org/tools/install). Once you do that, you can just use `cargo` as specified below.

```shell
$ git clone https://github.com/ackintosh/ray.git
$ cd ray
$ RUST_LOG=ray=info cargo run
```

### Architecture

![Architecture](https://raw.githubusercontent.com/ackintosh/ray/main/diagrams/architecture.png)

## Resources for Beacon Node implementers

Here are the specifications / documentations that Consensus Layer Implementers should refer to.

### Specs

- https://github.com/ethereum/consensus-specs
- https://github.com/ethereum/devp2p
- https://github.com/ethereum/beacon-metrics

### Documents

- https://github.com/timbeiko/eth-roadmap-faq

### Videos

- [Whiteboard Series with NEAR | Ep: 44 Adrian Manning from Sigma Prime](https://www.youtube.com/watch?v=XvWf6QMBO6k)  
  - > We'll explore how the nodes communicate and for what reason. This will include Discovery v5, libp2p, Gossipsub, and the Eth2 RPC.

### YouTube channels

- [Infinite Jungle](https://www.youtube.com/playlist?list=PLz3vbkrzRoXR_XIWVqnZcX11REeC6acN2) by Galaxy is a podcast about the evolution of Ethereum. Hosted weekly by Galaxy Research’s Christine Kim, the show dives into the ways Ethereum as an ecosystem is changing, covering broad trends and narratives that are shaping the way people use Ethereum and think about the value of Ethereum.


## Author

Authored and maintained by ackintosh.

> GitHub [@ackintosh](https://github.com/ackintosh) / Twitter [@NAKANO_Akihito](https://twitter.com/NAKANO_Akihito)