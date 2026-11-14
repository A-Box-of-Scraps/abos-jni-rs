package com.example;

public class TestOptionalInitializerFailure {
    static {
        if (System.currentTimeMillis() != 0) {
            throw new IllegalStateException("initializer must not be mistaken for a missing member");
        }
    }

    public static int present() {
        return 1;
    }
}
