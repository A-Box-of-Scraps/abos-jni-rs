package com.example;

public class TestOptionalMethodInitializerFailure {
    static {
        if (System.currentTimeMillis() != 0) {
            throw new NoSuchMethodError("dependency missing during initialization");
        }
    }

    public TestOptionalMethodInitializerFailure() {}

    public static int present() {
        return 1;
    }

    public int instancePresent() {
        return 1;
    }
}
