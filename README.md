# Ray

An Ethereum Consensus Layer implementation _for learning opportunities for to-be-implementers_.

[![CI](https://github.com/ackintosh/ray/actions/workflows/ci.yml/badge.svg)](https://github.com/ackintosh/ray/actions/workflows/ci.yml)

![banner image](https://raw.githubusercontent.com/ackintosh/ray/898488c66bf520a5df71a8d28c562b12355af9ee/banner.jpeg)

> Photo by [Marc-Olivier Jodoin](https://unsplash.com/@marcojodoin?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText) on [Unsplash](https://unsplash.com/?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText)

## Overview

This project is not intended for mainnet, provides developers with an opportunity to learn how to implement the Consensus Layer via running the binary on testnet or reading the source codes. 

We aim for a simpler implementation, by narrowing down the functions.

- Implemented for macOS
- Runs for Kiln testnet
- Simplicity over multifunctionality

## Resources for the Consensus Layer implementers

Here are the specifications / documentations that Consensus Layer Implementers should refer to.

### Specs

- https://github.com/ethereum/consensus-specs
- https://github.com/ethereum/devp2p

### Documents

- https://github.com/timbeiko/eth-roadmap-faq

## Getting started

### Running `ray`

```shell
$ git clone https://github.com/ackintosh/ray.git
$ cd ray
$ RUST_LOG=info cargo run
```
