package com.example;

public class ReferenceTypes {
    public static int calls;
    public String text = "original";
    public static String staticText = "original";
    public String[] strings;
    public static String[] staticStrings;
    public Comparable comparable;
    public static Comparable staticComparable;
    public Comparable[] comparables;
    public static Comparable[] staticComparables;
    public ReferenceTypes peer;

    public ReferenceTypes() {}

    public ReferenceTypes(Comparable value) {
        comparable = value;
        calls++;
    }

    public int acceptString(String value) {
        return ++calls;
    }

    public int acceptArray(String[] value) {
        return ++calls;
    }

    public int acceptMatrix(String[][] value) {
        return ++calls;
    }

    public int acceptBytes(byte[] value) {
        return ++calls;
    }

    public int acceptComparable(Comparable value) {
        return ++calls;
    }

    public int acceptComparables(Comparable[] value) {
        return ++calls;
    }

    public static int acceptStatic(Comparable value) {
        return ++calls;
    }

    public ReferenceTypes echo(ReferenceTypes value) {
        return value;
    }

    public String echoString(String value) {
        return value;
    }

    public String[] echoStrings(String[] value) {
        return value;
    }

    public Comparable echoComparable(Comparable value) {
        calls++;
        return value;
    }

    public static class Child extends ReferenceTypes {}
}
