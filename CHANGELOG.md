## [Unreleased]

### 增加

- 支持离线声音文件的转换。

## [0.1.2] - 2019-4-12

### 改变

- build.rs及Makefile 支持条件编译。
- 补上一个空的 read_stream。
- 把 close_stream 移到 Actor::stopped 中执行。

## [0.1.1] - 2019-3-7

### 增加

- 完成基本功能。等待联调。
- 增加 C main模块模拟调用。
- 引入微软的 wav reader，并修正其格式支持的问题。

## [0.1.0] - 2019-3-5

### 增加

- 以 [hss](https://github.com/garyhai/hss) 为框架快速搭建 ns_luis.
