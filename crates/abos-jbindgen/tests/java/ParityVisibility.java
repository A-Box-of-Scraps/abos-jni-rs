package com.example;

public class ParityVisibility {
    public int publicField = 11;
    protected int protectedField = 22;
    int packageField = 33;
    private int privateField = 44;

    public ParityVisibility() {}

    protected ParityVisibility(int value) {
        publicField = value;
    }

    ParityVisibility(String value) {
        publicField = value.length();
    }

    private ParityVisibility(boolean value) {}

    public int publicMethod() {
        return publicField;
    }

    protected int protectedMethod() {
        return protectedField;
    }

    int packageMethod() {
        return packageField;
    }

    private int privateMethod() {
        return privateField;
    }

    public native int publicNative();

    protected native int protectedNative();

    native int packageNative();

    private native int privateNative();
}
