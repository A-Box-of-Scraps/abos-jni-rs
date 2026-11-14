package com.example;

public class TestOptionalFieldInitializerFailure {
    static {
        if (System.currentTimeMillis() != 0) {
            throw new NoSuchFieldError("dependency missing during initialization");
        }
    }

    public static int present = 1;
    public int instancePresent = 1;
}
