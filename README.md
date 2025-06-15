# Kółko i Krzyżyk (Tic-Tac-Toe) – C#

Konsolowa gra w kółko-krzyżyk w języku **C#** na .NET 6.  
Pozwala grać w trybie dwóch graczy lub przeciwko prostemu AI (losowe ruchy).

## Uruchomienie

1. Zbuduj projekt:
    ```
    dotnet build
    ```
2. Uruchom:
    ```
    dotnet run
    ```

## Zasady
- Gra na planszy 3x3.
- Menu umożliwia wybór trybu: dwóch graczy lub gracz kontra komputer.
- Ruchy wykonuje się podając numer wiersza i kolumny.
- Wyniki mogą być zapisywane i wyświetlane (zależnie od pełnej implementacji).

## Struktura kodu
- `Program.cs` – start programu.
- `Controller.cs` – menu, logika rozgrywki.
- `Board.cs` – plansza.
- `Player.cs`, `HumanPlayer.cs`, `AIPlayer.cs` – obsługa graczy.
- `Cell.cs`, `GameEntity.cs` – elementy gry.

## Licencja
Projekt dostępny na licencji **Apache License 2.0** (szczegóły w pliku LICENSE.txt).
