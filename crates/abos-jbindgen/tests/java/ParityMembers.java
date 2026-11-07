package com.example;

public class ParityMembers {
    public static int shared = 5;
    public static final int CONSTANT = 19;
    public int value;
    public final int fixed;

    public ParityMembers(int value) {
        this.value = value;
        this.fixed = value + 1;
    }

    public int choose(int value) {
        return value + 10;
    }

    public int choose(String value) {
        return value.length() + 20;
    }

    public static int choose(int[] values) {
        return values.length + 30;
    }

    public static int choose(String[] values) {
        return values.length + 40;
    }

    public int[][] matrix(int[][] values) {
        return values;
    }
}
