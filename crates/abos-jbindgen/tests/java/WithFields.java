package com.example;

public class WithFields {
    public static final int CONSTANT = 42;
    public static String staticField = "static";

    public int publicField;
    private String privateField;
    protected double protectedField;

    public WithFields() {
        publicField = 0;
        privateField = "";
        protectedField = 0.0;
    }

    public int getPublicField() {
        return publicField;
    }

    public void setPublicField(int value) {
        publicField = value;
    }
}
