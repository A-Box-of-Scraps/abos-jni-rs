package com.type;

public class Identifiers {
    public int type;
    public int type_;
    public int non_null;
    public int café;

    public int match(int gen, int fooBar, int foo_bar, int env, int api) {
        return gen + 10 * fooBar + 100 * foo_bar + 1000 * env + 10000 * api;
    }

    public int gen() {
        return 7;
    }

    public int self() {
        return 8;
    }

    public int dollars(int $value) {
        return $value;
    }

    public int λ() {
        return 10;
    }

    public match echo(match value) {
        return value;
    }
}
