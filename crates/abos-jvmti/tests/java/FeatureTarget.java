public final class FeatureTarget {
    public long compute(int number, long wide, float fraction, double precise, Object object) {
        return number + wide + (long) fraction + (long) precise + (object == null ? 0 : 1000);
    }
}
