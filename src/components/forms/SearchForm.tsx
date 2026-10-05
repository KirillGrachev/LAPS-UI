import { useState, KeyboardEvent } from 'react';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { Search, Loader2, X } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';

interface SearchFormProps {
  initialQuery: string;
  onSubmit: (query: string) => void;
  isSearching: boolean;
  isRotating?: boolean;
  hasResult?: boolean;
  onClear?: () => void;
  translate: (key: string) => string;
}

/**
 * Форма поиска: контролируемый инпут (RHF для одного поля — оверкилл),
 * Enter отправляет, Esc очищает результат.
 */
export function SearchForm({
  initialQuery,
  onSubmit,
  isSearching,
  isRotating,
  hasResult,
  onClear,
  translate,
}: SearchFormProps) {
  const [query, setQuery] = useState(initialQuery);
  const isDisabled = isSearching || isRotating;

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape' && hasResult) {
      onClear?.();
    }
  };

  const handleSubmit = () => {
    if (isDisabled) return;
    onSubmit(query);
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        handleSubmit();
      }}
      className="flex flex-col gap-[8px] sm:gap-[10px]"
    >
      <label
        htmlFor="computerName"
        className="text-[0.75rem] sm:text-[0.8rem] font-bold text-muted-foreground uppercase tracking-tight text-center transition-colors"
      >
        {translate('search.computerNameLabel')}
      </label>
      <div className="flex flex-col sm:flex-row gap-[8px] sm:gap-[10px]">
        <div className="flex-1 flex flex-col gap-1">
          <Input
            id="computerName"
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isDisabled}
            className="w-full h-[40px] sm:h-[44px] font-bold text-center text-sm md:text-base uppercase disabled:opacity-75 disabled:cursor-not-allowed"
            placeholder={translate('search.computerNamePlaceholder')}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            maxLength={255}
          />
        </div>

        {hasResult ? (
          <Button
            type="button"
            variant="outline"
            onClick={(e) => {
              e.preventDefault();
              onClear?.();
              setQuery('');
            }}
            className="border-destructive/30 text-red-500 hover:bg-red-500/10 h-[40px] sm:h-[44px] sm:px-4 font-bold transition-all shadow-sm w-full sm:w-[220px] sm:min-w-[220px] sm:max-w-[220px] mt-1 sm:mt-0 overflow-hidden flex items-center justify-center gap-3"
          >
            <X className="w-5 h-5" />
            <span>{translate('app.clear')}</span>
          </Button>
        ) : (
          <Button
            type="submit"
            disabled={isDisabled || !query.trim()}
            className="bg-primary hover:bg-primary-hover text-primary-foreground h-[40px] sm:h-[44px] sm:px-4 font-bold transition-all shadow-sm w-full sm:w-[220px] sm:min-w-[220px] sm:max-w-[220px] mt-1 sm:mt-0 overflow-hidden flex items-center justify-center"
          >
            <AnimatePresence mode="wait">
              {isSearching ? (
                <motion.div
                  key="searching"
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="flex items-center justify-center gap-3 w-full"
                >
                  <Loader2 className="w-5 h-5 animate-spin" />
                  <span>{translate('search.searchingBtn')}</span>
                </motion.div>
              ) : (
                <motion.div
                  key="idle"
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="flex items-center justify-center gap-3 w-full"
                >
                  <Search className="w-5 h-5" />
                  <span>{translate('search.searchBtn')}</span>
                </motion.div>
              )}
            </AnimatePresence>
          </Button>
        )}
      </div>
    </form>
  );
}
