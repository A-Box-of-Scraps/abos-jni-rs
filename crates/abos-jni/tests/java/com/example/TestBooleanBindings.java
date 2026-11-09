package com.example;

public class TestBooleanBindings {
    public boolean value;
    public static boolean staticValue;

    public boolean echo(boolean input) {
        return input;
    }

    public static native boolean invertBound(boolean input);

    public static native boolean invertStandalone(boolean input);
}
