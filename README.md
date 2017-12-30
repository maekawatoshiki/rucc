# RUCC


[![](https://circleci.com/gh/maekawatoshiki/rucc/tree/master.svg?style=shield&circle-token=12276a02aa21f18324f9be74cbb922227b7c8551)](https://circleci.com/gh/maekawatoshiki/rucc)
[![](http://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

rucc is a small toy C compiler implemented in Rust while I'm learning Rust.

# REQUIREMENTS

- latest Rust (recommend [rustup](https://www.rustup.rs/))
- LLVM 4.0
```sh
# ubuntu, or debian...
$ apt-get install llvm-4.0 llvm-4.0-dev
```

# RUN

First, do test

```sh
$ cargo test
```

After the test exited successfully, you can try rucc easily with ``./rucc.sh``.

```sh
$ # slow (use binary created by `cargo build`)
$ ./rucc.sh [filename (*.c)]

$ # fast (use binary created by `cargo build --release`)
$ ./rucc.sh [filename (*.c)] --release
```

# FORK AND PULL REQUEST LIFECYCLE

1. fork https://github.com/maekawatoshiki/rucc repository
2. clone your repository on local pc

    ```sh
    $ git clone git@github.com:youraccount/rucc.git
    $ cd rucc
    ```

3. add maekawatoshiki upstream repository & fetch & confirm

    ```sh
    $ git remote add upstream git@github.com:maekawatoshiki/rucc.git
    $ git fetch upstream
    $ git branch -a

    * master
    remotes/origin/HEAD -> origin/master
    remotes/origin/master
    remotes/upstream/master
    ```

4. fetch & marge upstream

    ```sh
    $ git fetch upstream
    $ git merge upstream/master
    ```

5. pullrequest on github

# REFERENCES

I'm using [8cc](https://github.com/rui314/8cc) as reference.
