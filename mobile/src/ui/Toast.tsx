// A brief confirmation ("Copied", "Deleted" with Undo). Quiet: no stacking, no sound.
// Reduced motion keeps the fade and drops the slide (SPEC 21.5).

import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeIn, FadeOut, ReduceMotion } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { motion, radius, text, useTheme } from '../theme';
import { t } from '../strings';

type Show = (message: string, undo?: () => void) => void;
const ToastContext = createContext<Show>(() => {});

export function useToast(): Show {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const { c } = useTheme();
  const insets = useSafeAreaInsets();
  const [toast, setToast] = useState<{ id: number; message: string; undo?: () => void } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const show = useCallback<Show>((message, undo) => {
    clearTimeout(timer.current);
    setToast({ id: Date.now(), message, undo });
    timer.current = setTimeout(() => setToast(null), undo ? 4000 : 1600);
  }, []);
  return (
    <ToastContext.Provider value={show}>
      {children}
      {toast && (
        <Animated.View
          key={toast.id}
          entering={FadeIn.duration(motion.base).reduceMotion(ReduceMotion.System)}
          exiting={FadeOut.duration(motion.exit).reduceMotion(ReduceMotion.System)}
          style={[styles.wrap, { bottom: insets.bottom + 72 }]}
          pointerEvents="box-none"
          accessibilityLiveRegion="polite"
        >
          <View style={[styles.toast, { backgroundColor: c.selected }]}>
            <Text style={[text('bodyStrong'), { color: c.onSelected }]}>{toast.message}</Text>
            {toast.undo && (
              <Pressable
                onPress={() => {
                  toast.undo?.();
                  setToast(null);
                }}
                accessibilityRole="button"
                hitSlop={12}
              >
                <Text style={[text('bodyStrong'), { color: c.onSelectedAccent }]}>{t('action.undo')}</Text>
              </Pressable>
            )}
          </View>
        </Animated.View>
      )}
    </ToastContext.Provider>
  );
}

const styles = StyleSheet.create({
  wrap: { position: 'absolute', left: 16, right: 16, alignItems: 'center' },
  toast: {
    flexDirection: 'row',
    gap: 16,
    alignItems: 'center',
    paddingHorizontal: 18,
    paddingVertical: 12,
    borderRadius: radius.pill,
  },
});
