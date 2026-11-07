package com.example;

public class MixedMethods {
    public int call(int value) {
        return value + 1;
    }

    public native int call(long value);
}
