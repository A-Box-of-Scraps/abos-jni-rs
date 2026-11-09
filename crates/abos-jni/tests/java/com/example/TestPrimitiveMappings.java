package com.example;

public class TestPrimitiveMappings {
    public long value;
    public static long staticValue;

    public TestPrimitiveMappings(long value) {
        this.value = value;
    }

    public long echo(long value) {
        return value;
    }

    public static long echoStatic(long value) {
        return value;
    }
}
