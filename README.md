# 1-gegen-100

Party game variant of the 1 vs 100 gameshow.

## Terms

This software needs some standart terms

contestants -> All players that play against the challenger
challenger -> one player that plays against the contestants

## Round Logic

```mermaid
flowchart LR
    
    subgraph Round

        spieler-auswahl(Player selection)


        --> question(Question is Asked)
        --> gegner-antwort(Contestans Answer)
        --> spieler-antwort(Player answers)
        --> gegner-ausscheiden(Contestants Loose)
        --> gegner-uebrig{Contestants > 0?}
    
        gegner-uebrig -- no --> spieler-risiko{Player wants resolution}
        spieler-risiko -- yes --> spieler-richtig

        gegner-uebrig -- yes --> spieler-richtig{Players answer correct?}

        spieler-richtig -- yes --> win(Get points from Loosers)
        --> frage-ende{question == 10 or contestants == 0}

        frage-ende -- yes --> round-end
        frage-ende -- no --> question


        spieler-richtig -- no --> loose(Loose all points)
        --> round-end

    end


```