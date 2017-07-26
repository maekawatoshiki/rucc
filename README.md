# RUCC

[![](https://img.shields.io/travis/maekawatoshiki/rucc.svg?style=flat-square)](https://travis-ci.org/maekawatoshiki/rucc)
[![](http://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](./LICENSE)

- rucc is a small toy C compiler implemented in Rust
- developing to leran Rust

# REQUIREMENTS

- latest Rust(recommend [rustup](https://www.rustup.rs/))
- LLVM 3.8
```sh
# ubuntu, or debian...
$ apt-get install llvm-3.8
```

# RUN

- You had better use ``./rucc.sh``

```sh
$ # slow(cargo build)
$ ./rucc.sh [filename]
$ # fast(cargo build --release)
$ ./rucc.sh [filename] --release
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

- I'm using [8cc](https://github.com/rui314/8cc) as reference.
