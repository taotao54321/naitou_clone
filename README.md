# 内藤九段将棋秘伝 (FC) のクローン

## Build

```sh
$ git clone --recursive 'git@github.com:taotao54321/naitou_clone.git'
$ cd naitou_clone
$ cargo build --release
```

## Usage

USI プロトコルに対応している。[将棋所](http://shogidokoro.starfree.jp/) や
[ShogiGUI](http://shogigui.siganus.com/) で `target/release/naitou.exe` をエン
ジンとして登録する。

## Note

完全移植ではない。現状把握している相違点は以下の通り:

* 原作はプレイヤー側の詰み判定が出ても一部の指し手の生成/評価を続けるが、これは
  再現していない。
* 玉で王手できるバグは再現していない。
* 原作は候補手の評価値を修正する際に配列外参照を行っている箇所があるが、これは
  再現していない。

`src/bin/verify.rs` は再現性チェックツール(思考ログをエミュレータ上のそれと照合
する)。

`src/bin/solve.rs` は初期局面からの最短手順を求めるコードだが、現状では速度が遅
すぎて実用に耐えない(15 手全探索の場合、おそらく 1 年弱かかる)。
